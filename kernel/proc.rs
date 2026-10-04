use crate::kalloc::*;
use crate::memlayout::*;
use crate::memlayout::*;
use crate::param::*;
use crate::printk::*;
use crate::riscv::*;
use crate::sleeplock::*;
use crate::spinlock::*;
use crate::trampoline::*;
use crate::trap::*;
use crate::types::*;
use crate::vm::KernelPageTable;
use crate::vm::*;
use core::cell::UnsafeCell;
use core::ptr::{null, null_mut, NonNull};
use core::sync::atomic::AtomicBool;

pub type Pid = u32;

#[repr(C)]
pub struct Context {
    ra: usize,
    sp: usize,

    // calleeが保存する
    s0: usize,
    s1: usize,
    s2: usize,
    s3: usize,
    s4: usize,
    s5: usize,
    s6: usize,
    s7: usize,
    s8: usize,
    s9: usize,
    s10: usize,
    s11: usize,
}
impl Context {
    const fn new() -> Self {
        unsafe { core::mem::zeroed() }
    }
}

// cpuごとの状態
pub struct Cpu {
    pub proc: Option<NonNull<Proc>>, // このCPUで動いているプロセス
    pub context: Context,            // schedulerに遷移する時、ここにswtchする
    pub noff: u32,                   // push_offの深さ
    pub intena: bool,                // push_offの前に割り込みが許可されていたか
}
impl Cpu {
    const fn new() -> Self {
        Self {
            proc: None,
            context: Context::new(),
            noff: 0,
            intena: false,
        }
    }
}
pub struct CpuCell {
    val: UnsafeCell<Cpu>,
}
impl CpuCell {
    const fn new() -> Self {
        Self {
            val: UnsafeCell::new(Cpu::new()),
        }
    }
}
unsafe impl Sync for CpuCell {}

// TODO comment
#[derive(Clone, Copy)]
#[repr(C)]
pub struct TrapFrame {
    pub kernel_satp: usize,   // kernel page table
    pub kernel_sp: usize,     // プロセスのカーネルスタックのsp
    pub kernel_trap: usize,   // usertrap()
    pub epc: usize,           // user program counter
    pub kernel_hartid: usize, // kernel tp
    pub ra: usize,
    pub sp: usize,
    pub gp: usize,
    pub tp: usize,
    pub t0: usize,
    pub t1: usize,
    pub t2: usize,
    pub s0: usize,
    pub s1: usize,
    pub a0: usize,
    pub a1: usize,
    pub a2: usize,
    pub a3: usize,
    pub a4: usize,
    pub a5: usize,
    pub a6: usize,
    pub a7: usize,
    pub s2: usize,
    pub s3: usize,
    pub s4: usize,
    pub s5: usize,
    pub s6: usize,
    pub s7: usize,
    pub s8: usize,
    pub s9: usize,
    pub s10: usize,
    pub s11: usize,
    pub t3: usize,
    pub t4: usize,
    pub t5: usize,
    pub t6: usize,
}
impl TrapFrame {
    const fn new() -> Self {
        unsafe { core::mem::zeroed() }
    }
}

#[derive(PartialEq, Clone, Copy)]
pub enum ProcState {
    UNUSED,
    USED,
    SLEEPING,
    RUNNABLE,
    RUNNING,
    ZOMBIE,
}

pub type ExitCode = i32;
pub struct ProcShared {
    pub state: ProcState,
    pub chan: *const (),  // non-zeroなら、chanで眠っている
    pub killed: bool,     // trueなら、殺されている
    pub xstate: ExitCode, // exit status
}
impl ProcShared {
    pub const fn new() -> Self {
        Self {
            state: ProcState::UNUSED,
            chan: null_mut(),
            killed: false,
            xstate: 0,
        }
    }
}
pub struct ProcLocal {
    pub kstack: usize,             // kernel stackの仮想アドレス
    pub sz: usize,                 // process memoryのサイズ
    pub pagetable: PageTable,      // user page table
    pub trapframe: *mut TrapFrame, // tranpoline.Sのためのデータページ
    pub context: Context,          // processを動かすにはここにswtchする
    // TODO ofile
    // TODO inode
    pub name: &'static str,
}
impl ProcLocal {
    pub const fn new() -> Self {
        Self {
            kstack: 0,
            sz: 0,
            pagetable: PageTable::new(),
            trapframe: null_mut(),
            context: Context::new(),
            name: "",
        }
    }
}
pub struct Proc {
    pub pid: UnsafeCell<Pid>,            // process id
    pub lock: SpinLock<ProcShared>,      //
    pub parent: UnsafeCell<*const Proc>, // wait_lockが必要
    pub local: UnsafeCell<ProcLocal>,    // 自プロセスしかアクセスしないので、ロック不要
}
impl Proc {
    pub const fn new() -> Self {
        Self {
            pid: UnsafeCell::new(0),
            lock: SpinLock::new(ProcShared::new(), "process"),
            parent: UnsafeCell::new(null_mut()),
            local: UnsafeCell::new(ProcLocal::new()),
        }
    }
    pub fn alloc_pagetable(&self) -> bool {
        let mut pagetable = PageTable::new();
        if !pagetable.init() {
            return false;
        }
        unsafe {
            // trampoline codeをmapする。
            // system callにしか使わないため、PTE_Uではない
            if !pagetable.map(TRAMPOLINE, PG_SIZE, TRAMPOLINE_PHY as usize, PTE_R | PTE_X) {
                pagetable.free(0);
                return false;
            }
            // trapframeをmapする。
            if !pagetable.map(
                TRAPFRAME,
                PG_SIZE,
                (*self.local.get()).trapframe as usize,
                PTE_R | PTE_W,
            ) {
                pagetable.unmap(TRAMPOLINE, 1, false);
                pagetable.free(0);
                return false;
            }
            (*self.local.get()).pagetable = pagetable;
        }
        true
    }
    pub fn free_pagetable(&self) {
        unsafe {
            (*self.local.get()).pagetable.unmap(TRAMPOLINE, 1, false);
            (*self.local.get()).pagetable.unmap(TRAPFRAME, 1, false);
            (*self.local.get()).pagetable.free((*self.local.get()).sz);
        }
    }
    pub fn free(&self, shared: Guard<'static, ProcShared, SpinLock<ProcShared>>) {
        unsafe {
            if (*self.local.get()).trapframe != null_mut() {
                kfree(NonNull::new_unchecked(
                    (*self.local.get()).trapframe as *mut u8,
                ));
            }
            if (*self.local.get()).pagetable.root != null_mut() {
                self.free_pagetable();
            }
            *shared.data = ProcShared::new();
            let saved = (*self.local.get()).kstack;
            *self.local.get() = ProcLocal::new();
            (*self.local.get()).kstack = saved;
            *self.pid.get() = 0;
        }
    }
    pub fn growproc(&self, n: isize) -> bool {
        unsafe {
            let mut sz = (*self.local.get()).sz;
            if n > 0 {
                if sz + n as usize > TRAPFRAME {
                    return false;
                }
                if {
                    sz = (*self.local.get())
                        .pagetable
                        .alloc(sz, sz + n as usize, PTE_W);
                    sz == 0
                } {
                    return false;
                }
            } else if n < 0 {
                sz = (*self.local.get())
                    .pagetable
                    .dealloc(sz, sz.wrapping_add_signed(n));
            }
            (*self.local.get()).sz = sz;
            true
        }
    }
    // selfの孤児をinitprocに紐付ける
    // callerはwait_lockを取っていないといけない
    pub fn reparent(&self) {
        for pp in &PROC {
            unsafe {
                if *pp.parent.get() == core::ptr::from_ref(self) {
                    *pp.parent.get() = INITPROC;
                    wakeup(core::ptr::from_ref(INITPROC).cast::<()>());
                }
            }
        }
    }
    pub fn setkilled(&self) {
        let mut shared = self.lock.acquire();
        shared.killed = true;
    }
    pub fn killed(&self) -> bool {
        let mut shared = self.lock.acquire();
        shared.killed
    }
}
unsafe impl Sync for Proc {}

static CPUS: [CpuCell; NCPU] = [const { CpuCell::new() }; NCPU];

pub static PROC: [Proc; NPROC] = [const { Proc::new() }; NPROC];

static INITPROC: &Proc = &PROC[0];

static PID_LOCK: SpinLock<Pid> = SpinLock::new(1, "PID_LOCK");

//trampoline, forkret, freeproc

static WAIT_LOCK: SpinLock<Pid> = SpinLock::new(1, "WAIT_LOCK");

// カーネルスタックのためのページを割り当てる
// メモリ中の高い場所に起き、ガードページが続く
pub fn proc_mapstacks(kpgtbl: &KernelPageTable) {
    for (i, p) in PROC.iter().enumerate() {
        match kalloc().map(NonNull::as_ptr) {
            None => panic!("kalloc"),
            Some(pa) => {
                kpgtbl.map(kstack(i), pa as usize, PG_SIZE, PTE_R | PTE_W);
            }
        }
    }
}

pub fn proc_init() {
    for (i, p) in PROC.iter().enumerate() {
        unsafe {
            (*p.local.get()).kstack = kstack(i);
        }
    }
}

pub fn cpu_id() -> u8 {
    r_tp() as u8
}

pub fn my_cpu() -> *mut Cpu {
    CPUS[cpu_id() as usize].val.get()
}

// このcpuで動いているprocを取得
// procが割り当てられていなければpanicする
pub fn my_proc() -> &'static Proc {
    unsafe {
        push_off();
        let c: *const Cpu = my_cpu();
        let p: *const Proc = (*c).proc.expect("my_cpu").as_ptr();
        pop_off();
        &*p
    }
}

// 新しいpidを取ってくる
fn alloc_pid() -> Pid {
    unsafe {
        let mut nextpid = PID_LOCK.acquire();
        let pid = *nextpid;
        *nextpid = *nextpid + 1;
        pid
    }
}

// PROCの中にUNUSEDプロセスがないかを探す。
// 見つかったら初期化してp->lockを取った状態で返す
fn alloc_proc() -> Option<(
    &'static Proc,
    Guard<'static, ProcShared, SpinLock<ProcShared>>,
)> {
    let (p, mut share) = 'find: {
        for p in &PROC {
            let mut share = p.lock.acquire();
            if share.state == ProcState::UNUSED {
                break 'find (p, share);
            }
        }
        return None;
    };

    unsafe {
        *p.pid.get() = alloc_pid();
        share.state = ProcState::USED;

        (*p.local.get()).trapframe = match kalloc().map(NonNull::as_ptr) {
            Some(mem) => mem as *mut TrapFrame,
            None => {
                p.free(share);
                return None;
            }
        };

        if !p.alloc_pagetable() {
            p.free(share);
            return None;
        };

        // forkretから実行を始め、user spaceに帰るための設定
        (*p.local.get()).context = Context::new();
        (*p.local.get()).context.ra = forkret as *const () as usize;
        (*p.local.get()).context.sp = (*p.local.get()).kstack + PG_SIZE;
    }

    return Some((p, share));
}

// temporary
pub static USER_IMAGE: &[u8] = include_bytes!("../build/user/iocheck.elf");
use crate::test_iocheck::*;
// first user processをsetup
pub fn user_init() {
    unsafe {
        let mut share = INITPROC.lock.acquire();
        *INITPROC.pid.get() = 1;
        share.state = ProcState::RUNNABLE;
        (*INITPROC.local.get()).trapframe = kalloc().expect("user_init").as_ptr() as *mut TrapFrame;

        if !(*INITPROC).alloc_pagetable() {
            panic!("user_init");
        }

        // TODO cwdとかの設定

        // forkretから実行を始め、user spaceに帰るための設定
        (*INITPROC.local.get()).context = Context::new();
        (*INITPROC.local.get()).context.ra = forkret as *const () as usize;
        (*INITPROC.local.get()).context.sp = (*(*INITPROC).local.get()).kstack + PG_SIZE;

        // temporary
        let local = &mut *INITPROC.local.get();
        let (entry, image_end) = load_init_image(&local.pagetable);
        let stack_base = pgroundup(image_end);
        let stack_top = stack_base + PG_SIZE;
        local.pagetable.alloc(image_end, stack_top, PTE_W);
        local.sz = stack_top;
        local.name = "iocheck";
        core::ptr::write_bytes(local.trapframe, 0, 1);
        (*local.trapframe).epc = entry;
        (*local.trapframe).sp = stack_top;
    }
}

// 親をコピーして新しいプロセスを作る。
// fork()から返ったかのように子のkernel stackを作り上げる
fn kfork() -> Option<Pid> {
    let p = my_proc();

    let (np, shared) = match alloc_proc() {
        None => return None,
        Some(pid) => pid,
    };

    unsafe {
        if !(*p.local.get())
            .pagetable
            .copy(&(*np.local.get()).pagetable, (*p.local.get()).sz)
        {
            np.free(shared);
            return None;
        }

        (*np.local.get()).sz = (*p.local.get()).sz;

        // saved user registriesをコピー
        *(*np.local.get()).trapframe = *(*p.local.get()).trapframe;

        // forkから0が返ったように見せる
        (*(*np.local.get()).trapframe).a0 = 0;

        // TODO NOFILE
        // TODO cwd

        (*np.local.get()).name = (*p.local.get()).name;

        let pid = *np.pid.get();

        drop(shared);

        let guard = WAIT_LOCK.acquire();
        *np.parent.get() = p;
        drop(guard);

        let mut share = np.lock.acquire();
        share.state = ProcState::RUNNABLE;

        Some(pid)
    }
}

// 現在のプロセスから抜ける。
// 抜けたプロセスはzonbieで、親のwaitが呼ばれるまで残る
pub fn kexit(status: ExitCode) -> ! {
    let p = my_proc();
    if core::ptr::eq(p, INITPROC) {
        panic!("init exiting");
    }

    // TODO すべてのファイルを閉じる

    // TODO cwdに関する一連の操作

    let mut lock = WAIT_LOCK.acquire();
    p.reparent();
    unsafe {
        wakeup((*p.parent.get()).cast::<()>());
    }

    let mut share = p.lock.acquire();
    share.xstate = status;
    share.state = ProcState::ZOMBIE;
    drop(lock);

    // schedulerに飛び、戻らない
    sched(share);
    panic!("zombie exit");
}

// 子のプロセスがexitするのを待ち、pidを返す
pub fn kwait(addr: usize) -> Option<ExitCode> {
    let p = my_proc();
    let mut lock = WAIT_LOCK.acquire();

    loop {
        let mut have_kids = false;
        unsafe {
            // exit childrenを探すためにtableをスキャンする
            for pp in &PROC {
                if *pp.parent.get() == p {
                    let mut share = pp.lock.acquire();
                    have_kids = true;
                    if share.state == ProcState::ZOMBIE {
                        let pid = *pp.pid.get();
                        if addr != 0
                            && !copyout(
                                &(*p.local.get()).pagetable,
                                (*p.local.get()).sz,
                                addr,
                                &raw const share.xstate as usize,
                                size_of_val(&share.xstate),
                            )
                        {
                            return None;
                        }
                        *pp.parent.get() = null_mut();
                        pp.free(share);
                        return Some(pid as ExitCode);
                    }
                }
            }
            if !have_kids || p.killed() {
                return None;
            }

            sleep_prepare(core::ptr::from_ref(p) as *const ());
            drop(lock);
            sleep();
            lock = WAIT_LOCK.acquire();
        }
    }
}

unsafe extern "C" {
    fn swtch(old: *mut Context, new: *const Context);
}

// CPUごとのプロセススケジューラ
// 各CPUはセットアップが終了次第
// スケジューラは返らず、
// - プロセスを選ぶ
// - そのプロセスを走らせる
// - 最終的にそのプロセスが戻ってくる
pub fn scheduler() {
    let c = my_cpu();

    unsafe {
        (*c).proc = None;
    }

    loop {
        // schedulerの前のプロセスが割り込みを禁止していた場合、ずっとtrap vectorが実行されない
        // すべてのプロセスがtrapによる処理を待っていた場合、何かが呼び出されることがなくなる
        // 一度intr_onしてたまっているinterruptのtrap処理を行った後、再度RUNNABLEになったプロセスを実行していく
        // その後、RUNNABLEかどうかを見るループ中に割り込みが走ってRUNNABLEになると、lost wakeupが発生しうる
        // そのため、再びintr_offする
        intr_on();
        intr_off();
        let mut found = false;

        unsafe {
            for p in &PROC {
                let mut share = p.lock.acquire();
                if share.state == ProcState::RUNNABLE {
                    // 選んだプロセスに移る
                    // processのロックを開放し、
                    // schedに戻る前に再度取得するのはprocess側の仕事
                    share.state = ProcState::RUNNING;
                    (*c).proc = Some(NonNull::from(p));

                    swtch(&raw mut (*c).context, &raw const (*p.local.get()).context);

                    // switch先のintenaを引き継がず、割り込みは禁止したまま
                    (*c).intena = false;

                    // プロセスは完了した
                    // pのstateは変わっているかもしれない
                    (*c).proc = None;

                    found = true;
                }
            }
            if !found {
                core::arch::asm!("wfi");
            }
        }
    }
}

// schedulerに移る。pのロックのみを保持しており、p.stateが変えられた状態で呼ぶ。
// 引数としてguardを得る。
// このguardのlockは、swtchでschedulerに渡った後、向こう側の別のguardにより開放される。
// その後、向こうから来たlockはこちらのguardのdropで開放される
// intenaはこのこのカーネルスレッドの値でありCPUの値ではないので、変更する。
// 本来ならproc.intenaであるべきだが、ロックがあるがプロセスがない状況で壊れる
fn sched(shared: Guard<'static, ProcShared, SpinLock<ProcShared>>) {
    let p = my_proc();
    unsafe {
        if !p.lock.holding() {
            panic!("sched p->lock");
        }
        if (*my_cpu()).noff != 1 {
            panic!("sched locks");
        }
        if shared.state == ProcState::RUNNING {
            panic!("sched RUNNING");
        }
        if intr_get() {
            panic!("sched interruptible");
        }
        let intena = (*my_cpu()).intena;

        swtch(
            &raw mut (*p.local.get()).context,
            &raw const (*my_cpu()).context,
        );

        (*my_cpu()).intena = intena;
    }
}

// one scheduling roundについてCPUを解放
// xv6ではyield
pub fn yield_cpu() {
    let p = my_proc();
    unsafe {
        let mut shared = p.lock.acquire();
        shared.state = ProcState::RUNNABLE;
        sched(shared);
    }
}

// フォークで作られた子が最初にschedulerから呼ばれた時、
// swtchがreturnする先はforkret
extern "C" fn forkret() {
    let p = my_proc();
    unsafe {
        let mut shared = p.lock.reacquire();
        // schedulerで確保されたロックはこのブロックが終わる時点で開放される
    }
    static mut FIRST: bool = true;
    unsafe {
        if FIRST {
            // ファイルシステム初期化はsleepを使うため通常プロセスのコンテキストで実行されないといけない。
            // このあと/initを呼び出す都合で、実行できる機会はここしかない
            // TODO fsinit
            FIRST = false;
        }
    }

    // usertrapの値を偽装して、ユーザ空間に帰る
    prepare_return();
    unsafe {
        let satp = make_satp(&(*p.local.get()).pagetable);
        let trampoline_userret = TRAMPOLINE + (USERRET as usize - TRAMPOLINE_PHY as usize);
        let userret_fn: unsafe extern "C" fn(usize) -> ! = core::mem::transmute(trampoline_userret);
        userret_fn(satp);
    }
}

// 現在のプロセスを、waiting for wakeup on chanとして登録
pub fn sleep_prepare(chan: *const ()) {
    let p = my_proc();
    let mut shared = p.lock.acquire();
    if chan == null_mut() {
        panic!("sleep_prepare: zero chan");
    }
    shared.chan = chan;
}

// threadをsleepにする。sleep_prepareが前に行われていることを前提とする
// これ以前にchanがwakeupされていたら、眠らない
pub fn sleep() {
    let p = my_proc();
    let mut shared = p.lock.acquire();
    if shared.chan != null_mut() {
        shared.state = ProcState::SLEEPING;
        sched(shared);
    }
}

// あるchanで眠っているプロセスをすべて起動する
pub fn wakeup(chan: *const ()) {
    for p in &PROC {
        let mut shared = p.lock.acquire();
        if shared.chan == chan {
            shared.chan = null_mut();
            if shared.state == ProcState::SLEEPING {
                shared.state = ProcState::RUNNABLE;
            }
        }
    }
}

// 与えられたpidのプロセスを殺す
// user spaceに帰ろうとしたタイミングで殺される
pub fn kkill(pid: Pid) -> bool {
    for p in &PROC {
        let mut shared = p.lock.acquire();
        unsafe {
            if *p.pid.get() == pid {
                shared.killed = true;
                if shared.state == ProcState::SLEEPING {
                    // プロセスを起こす
                    shared.state = ProcState::RUNNABLE;
                }
                return true;
            }
        }
    }
    false
}

// usr_dstの値により、user adressかkernel addressに値をコピー
// 成功したらtrue
pub fn either_copyout(user_dst: bool, dst: usize, src: usize, len: usize) -> bool {
    unsafe {
        if user_dst {
            let p = my_proc();
            copyout(
                &(*p.local.get()).pagetable,
                (*p.local.get()).sz,
                dst,
                src,
                len,
            )
        } else {
            core::ptr::copy(src as *const u8, dst as *mut u8, len);
            true
        }
    }
}

// usr_dstの値により、user adressかkernel addressから値をコピー
// 成功したらtrue
pub fn either_copyin(dst: usize, user_src: bool, src: usize, len: usize) -> bool {
    unsafe {
        if user_src {
            let p = my_proc();
            copyin(
                &(*p.local.get()).pagetable,
                (*p.local.get()).sz,
                dst,
                src,
                len,
            )
        } else {
            core::ptr::copy(src as *const u8, dst as *mut u8, len);
            true
        }
    }
}

// デバッグ用に、processのlistを出力
// consoleで^Pが入力されたときに走る
// ロックは取らない
pub fn procdump() {
    let states: [&str; 6] = ["unused", "used", "sleep ", "runble", "run   ", "zombie"];
    printk!("\n");
    for p in &PROC {
        unsafe {
            let state = (*p.lock.acquire_without_lock()).state;
            if state == ProcState::UNUSED {
                continue;
            }
            let state = states[state as usize];
            printk!("{} {} {}\n", *p.pid.get(), state, (*p.local.get()).name);
        }
    }
}
