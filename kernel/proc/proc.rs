use super::{kalloc::*, pagetable::*, spinlock::*, trampoline::*, trap::*, vm::*};
use crate::{
    fs::{file::*, fs::*, inode::*, log::*},
    memlayout::*,
    riscv::*,
    user::exec::*,
    utils::printk::*,
};
use api::param::*;
use api::vm::*;
use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::ptr::{NonNull, null, null_mut};
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

/// cpuごとの状態
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

/// TODO comment
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
    pub kstack: Page,                        // kernel stackの仮想アドレス
    pub sz: usize,                           // process memoryのサイズ
    pub pagetable: PageTable,                // user page table
    pub trapframe: *mut TrapFrame,           // tranpoline.Sのためのデータページ
    pub context: Context,                    // processを動かすにはここにswtchする
    pub ofile: [Option<FileHandle>; NOFILE], // 開いているファイルのハンドル
    pub cwd: InodeHandle,
    pub name: &'static str,
    pub name_inner: [u8; MAX_PATH],
}
impl ProcLocal {
    pub const fn new() -> Self {
        Self {
            kstack: Page(VAddr(0)),
            sz: 0,
            pagetable: PageTable::new(),
            trapframe: null_mut(),
            context: Context::new(),
            ofile: [const { None }; NOFILE],
            cwd: InodeHandle::new(),
            name: &"",
            name_inner: [0; MAX_PATH],
        }
    }
}
pub struct Proc {
    pub pid: UnsafeCell<Pid>,            // process id
    pub lock: SpinLock<ProcShared>,      //
    pub parent: UnsafeCell<*const Proc>, // wait_lockが必要
    pub local: UnsafeCell<ProcLocal>,    // 自プロセスしかアクセスしないので、ロック不要
}
impl Deref for Proc {
    type Target = ProcLocal;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.local.get() }
    }
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
    pub fn alloc_pagetable(&self) -> Option<PageTable> {
        let mut pagetable = PageTable::new();
        if !pagetable.init() {
            return None;
        }
        unsafe {
            // trampoline codeをmapする。
            // system callにしか使わないため、PteFlag::U as usizeではない
            if !pagetable.map(
                ARange::span(TRAMPOLINE, 1),
                ARange::span(TRAMPOLINE_PHY(), 1),
                PteFlag::R as u32 | PteFlag::X as u32,
            ) {
                pagetable.free(0);
                return None;
            }
            // trapframeをmapする。
            if !pagetable.map(
                ARange::span(TRAPFRAME, 1),
                ARange::span(Frame(PAddr(self.trapframe as usize)), 1),
                PteFlag::R as u32 | PteFlag::W as u32,
            ) {
                pagetable.unmap(ARange::span(TRAMPOLINE, 1), false);
                pagetable.free(0);
                return None;
            }
        }
        unsafe { Some(pagetable) }
    }
    pub fn free(&self, shared: &mut Guard<'static, ProcShared, SpinLock<ProcShared>>) {
        unsafe {
            if self.trapframe != null_mut() {
                kfree(Frame(PAddr(self.trapframe as usize)));
            }
            if self.pagetable.root != null_mut() {
                self.pagetable.free_all(self.sz);
            }
            **shared = ProcShared::new();
            let saved = self.kstack;
            (*self.local.get()) = ProcLocal::new();
            (*self.local.get()).kstack = saved;
            *self.pid.get() = 0;
        }
    }
    pub fn growproc(&self, n: isize) -> bool {
        unsafe {
            let mut sz = self.sz;
            if n > 0 {
                if sz + n as usize > TRAPFRAME.raw() {
                    return false;
                }
                sz = match self.pagetable.alloc(sz, sz + n as usize, PteFlag::W as u32) {
                    Some(sz) if sz != 0 => sz,
                    _ => return false,
                };
            } else if n < 0 {
                sz = self.pagetable.dealloc(sz, sz.wrapping_add_signed(n));
            }
            (*self.local.get()).sz = sz;
            true
        }
    }
    /// selfの孤児をinitprocに紐付ける
    pub fn reparent(&self, wait_lock: &Guard<'static, u8, SpinLock<u8>>) {
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

static PID_LOCK: SpinLock<Pid> = SpinLock::new(2, "PID_LOCK");

//trampoline, forkret, freeproc
static WAIT_LOCK: SpinLock<u8> = SpinLock::new(1, "WAIT_LOCK");

/// カーネルスタックのためのページを割り当てる
/// メモリ中の高い場所に起き、ガードページが続く
pub fn proc_mapstacks(kpgtbl: &KernelPageTable) {
    for (i, p) in PROC.iter().enumerate() {
        match kalloc() {
            None => panic!("kalloc"),
            Some(frame) => {
                kpgtbl.map(
                    ARange::span(kstack(i), 1),
                    ARange::span(frame, 1),
                    PteFlag::R as u32 | PteFlag::W as u32,
                );
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

/// このcpuで動いているprocを取得
/// procが割り当てられていなければpanicする
pub fn my_proc() -> &'static Proc {
    unsafe {
        push_off();
        let c: *const Cpu = my_cpu();
        let p: *const Proc = (*c).proc.expect("my_cpu").as_ptr();
        pop_off();
        &*p
    }
}

/// 新しいpidを取ってくる
fn alloc_pid() -> Pid {
    unsafe {
        let mut nextpid = PID_LOCK.acquire();
        let pid = *nextpid;
        *nextpid = *nextpid + 1;
        pid
    }
}

/// PROCの中にUNUSEDプロセスがないかを探す。
/// 見つかったら初期化してp->lockを取った状態で返す
fn alloc_proc() -> Option<(
    &'static Proc,
    Guard<'static, ProcShared, SpinLock<ProcShared>>,
)> {
    let Some((p, mut share)) = PROC.iter().find_map(|p| {
        let mut share = p.lock.acquire();
        if share.state == ProcState::UNUSED {
            Some((p, share))
        } else {
            None
        }
    }) else {
        return None;
    };

    unsafe {
        *p.pid.get() = alloc_pid();
        share.state = ProcState::USED;

        (*p.local.get()).trapframe = match kalloc() {
            Some(mem) => mem.as_mut_ptr() as *mut TrapFrame,
            None => {
                p.free(&mut share);
                return None;
            }
        };

        let Some(pagetable) = p.alloc_pagetable() else {
            p.free(&mut share);
            return None;
        };
        (*p.local.get()).pagetable = pagetable;

        // forkretから実行を始め、user spaceに帰るための設定
        (*p.local.get()).context = Context::new();
        (*p.local.get()).context.ra = forkret as *const () as usize;
        (*p.local.get()).context.sp = (p.kstack + 1).raw();
    }

    return Some((p, share));
}

/// first user processをsetup
pub fn user_init() {
    unsafe {
        let mut share = INITPROC.lock.acquire();
        *INITPROC.pid.get() = 1;
        share.state = ProcState::RUNNABLE;
        drop(share);
        (*INITPROC.local.get()).trapframe = kalloc().expect("user_init").as_ptr() as *mut TrapFrame;

        let Some(pagetable) = (*INITPROC).alloc_pagetable() else {
            panic!("user_init");
        };
        (*INITPROC.local.get()).pagetable = pagetable;

        // forkretから実行を始め、user spaceに帰るための設定
        (*INITPROC.local.get()).context = Context::new();
        (*INITPROC.local.get()).context.ra = forkret as *const () as usize;
        (*INITPROC.local.get()).context.sp = (INITPROC.kstack + 1).raw();

        let handle = LogHandle::new();
        (*INITPROC.local.get()).cwd = namei(b"/", &handle).unwrap();
        drop(handle);
    }
}

pub fn copy_name<'a>(dst: &'a mut [u8], src: &[u8]) -> &'a str {
    let mut len = 0;
    for chunk in src.utf8_chunks() {
        let valid = chunk.valid().as_bytes();
        dst[len..len + valid.len()].copy_from_slice(valid);
        len += valid.len();

        if !chunk.invalid().is_empty() {
            dst[len] = b'?';
            len += 1;
        }
    }
    core::str::from_utf8(&dst[..len]).unwrap()
}

/// 親をコピーして新しいプロセスを作る。
/// fork()から返ったかのように子のkernel stackを作り上げる
pub fn kfork() -> Option<Pid> {
    let p = my_proc();

    let (np, mut shared) = match alloc_proc() {
        None => return None,
        Some(pid) => pid,
    };

    unsafe {
        if !p.pagetable.copy(&np.pagetable, p.sz) {
            np.free(&mut shared);
            return None;
        }

        (*np.local.get()).sz = p.sz;

        // saved user registriesをコピー
        *(*np.local.get()).trapframe = *p.trapframe;

        // forkから0が返ったように見せる
        (*(*np.local.get()).trapframe).a0 = 0;

        for (i, f_raw) in p.ofile.iter().enumerate() {
            if let Some(f) = f_raw {
                (*(*np).local.get()).ofile[i] = Some(f.clone());
            }
        }
        (*(*np).local.get()).cwd = p.cwd.clone();

        (*np.local.get()).name = copy_name(&mut (*np.local.get()).name_inner, &p.name.as_bytes());

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

/// 現在のプロセスから抜ける。
/// 抜けたプロセスはzonbieで、親のwaitが呼ばれるまで残る
pub fn kexit(status: ExitCode) -> ! {
    let p = my_proc();
    assert!(!core::ptr::eq(p, INITPROC), "init exiting");

    unsafe {
        for i in 0..NOFILE {
            // Noneを代入すると自動でDropされる
            (*p.local.get()).ofile[i] = None;
        }

        let handle = LogHandle::new();
        let cwd = core::ptr::replace(&raw mut (*p.local.get()).cwd, InodeHandle::new());
        cwd.drop(&handle);
    }

    let mut lock = WAIT_LOCK.acquire();
    p.reparent(&lock);
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

/// 子のプロセスがexitするのを待ち、pidを返す
/// 子プロセスの終了ステータスはaddrに書き込まれる
pub fn kwait(addr: VAddr) -> Option<u32> {
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
                        if addr != VAddr(0)
                            && !copyout(
                                &p.pagetable,
                                p.sz,
                                ARange::span(addr, size_of_val(&share.xstate)),
                                &share.xstate.to_ne_bytes(),
                            )
                        {
                            return None;
                        }
                        *pp.parent.get() = null_mut();
                        pp.free(&mut share);
                        return Some(pid);
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

/// CPUごとのプロセススケジューラ
/// 各CPUはセットアップが終了次第
/// スケジューラは返らず、
/// - プロセスを選ぶ
/// - そのプロセスを走らせる
/// - 最終的にそのプロセスが戻ってくる
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

/// schedulerに移る。pのロックのみを保持しており、p.stateが変えられた状態で呼ぶ。
/// 引数としてguardを得る。
/// このguardのlockは、swtchでschedulerに渡った後、向こう側の別のguardにより開放される。
/// その後、向こうから来たlockはこちらのguardのdropで開放される
/// intenaはこのこのカーネルスレッドの値でありCPUの値ではないので、変更する。
/// 本来ならproc.intenaであるべきだが、ロックがあるがプロセスがない状況で壊れる
fn sched(shared: Guard<'static, ProcShared, SpinLock<ProcShared>>) {
    let p = my_proc();
    unsafe {
        assert!(p.lock.holding(), "sched p->lock");
        assert!((*my_cpu()).noff == 1, "sched locks");
        assert!(shared.state != ProcState::RUNNING, "sched RUNNING");
        assert!(!intr_get(), "sched interruptible");

        let intena = (*my_cpu()).intena;

        swtch(
            &raw mut (*p.local.get()).context,
            &raw const (*my_cpu()).context,
        );

        (*my_cpu()).intena = intena;
    }
}

/// one scheduling roundについてCPUを解放
/// xv6ではyield
pub fn yield_cpu() {
    let p = my_proc();
    unsafe {
        let mut shared = p.lock.acquire();
        shared.state = ProcState::RUNNABLE;
        sched(shared);
    }
}

/// フォークで作られた子が最初にschedulerから呼ばれた時、
/// swtchがreturnする先はforkret
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
            fs_init(ROOT_DEV);
            FIRST = false;
            let argc = kexec(b"/bin/init", &[b"/bin/init"]);
            assert!(argc != -1, "exec");
            (*p.trapframe).a0 = argc as usize;
        }
    }

    // usertrapの値を偽装して、ユーザ空間に帰る
    prepare_return();
    unsafe {
        let satp = make_satp(&p.pagetable);
        let trampoline_userret: VAddr =
            TRAMPOLINE.addr() + (USERRET().raw() - TRAMPOLINE_PHY().raw());
        let userret_fn: unsafe extern "C" fn(usize) -> ! =
            core::mem::transmute(trampoline_userret.raw());
        userret_fn(satp);
    }
}

/// 現在のプロセスを、waiting for wakeup on chanとして登録
pub fn sleep_prepare(chan: *const ()) {
    let p = my_proc();
    let mut shared = p.lock.acquire();
    assert!(chan != null_mut(), "sleep_prepare: zero chan");
    shared.chan = chan;
}

/// threadをsleepにする。sleep_prepareが前に行われていることを前提とする
/// これ以前にchanがwakeupされていたら、眠らない
pub fn sleep() {
    let p = my_proc();
    let mut shared = p.lock.acquire();
    if shared.chan != null_mut() {
        shared.state = ProcState::SLEEPING;
        sched(shared);
    }
}

/// あるchanで眠っているプロセスをすべて起動する
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

/// 与えられたpidのプロセスを殺す
/// user spaceに帰ろうとしたタイミングで殺される
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

/// usr_dstの値により、kernel spaceからuser adressかkernel addressに値をコピー
/// 成功したらtrue
pub fn either_copyout(user_dst: bool, mut dst: ARange<VAddr>, src: &[u8]) -> bool {
    assert!(dst.len() == src.len());
    if user_dst {
        let p = my_proc();
        copyout(&p.pagetable, p.sz, dst, src)
    } else {
        dst.copy_from_slice(src);
        true
    }
}

/// usr_dstの値により、user adressかkernel addressからkernel spaceに値をコピー
/// 成功したらtrue
pub fn either_copyin(dst: &mut [u8], user_src: bool, src: ARange<VAddr>) -> bool {
    assert!(dst.len() == src.len());
    unsafe {
        if user_src {
            let p = my_proc();
            copyin(&p.pagetable, p.sz, dst, src)
        } else {
            dst.copy_from_slice(&src);
            true
        }
    }
}

/// デバッグ用に、processのlistを出力
/// consoleで^Pが入力されたときに走る
/// ロックは取らない
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
            printk!("{} {} {}\n", *p.pid.get(), state, p.name);
        }
    }
}
