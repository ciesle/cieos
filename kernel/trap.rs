use crate::memlayout::*;
use crate::plic::*;
use crate::proc::*;
use crate::riscv::*;
use crate::spinlock::*;
use crate::syscall::*;
use crate::trampoline::*;
use crate::uart::*;

static TICKS_LOCK: SpinLock<u64> = SpinLock::new(0, "time");

unsafe extern "C" {
    fn kernelvec();
}

// kernelで例外とtrapを処理するための設定
pub fn trap_init_hart() {
    w_stvec(kernelvec as *const () as usize);
}

// 割り込み、例外、ユーザ空間からのシステムコールを担当
// trampoline.Sから呼ばれ、そこに返る
// trampoline.Sがスイッチするためのuser satpが返り値
extern "C" fn usertrap() -> usize {
    if (r_sstatus() & SSTATUS_SPP) != 0 {
        panic!("usertrap: not from user mode");
    }

    // 割り込みと例外をkerneltrap()に送る
    w_stvec(kernelvec as *const () as usize);

    let p = my_proc();
    let mut which_dev = 0;

    unsafe {
        // user program counterを保存
        (*(*p.local.get()).trapframe).epc = r_sepc();

        //system call
        if r_scause() == 8 {
            if p.killed() {
                kexit(-1);
            }

            // sepcはecall命令を指しているはずだが、
            // 返る先はその次の命令であってほしい
            (*(*p.local.get()).trapframe).epc += 4;

            // 割り込みはsepc, scause, sstatusを変化させる。
            // 保存を終えた今のタイミングなら割り込みを許可できる
            intr_on();

            syscall();
        } else if {
            which_dev = devintr();
            which_dev
        } != 0
        {
        } else {
            printk!(
                "usertrap(): unexpected scause 0x{:x} pid = {}\n",
                r_scause(),
                *p.pid.get()
            );
            printk!(
                "          : sepc = 0x{:x} stval = 0x{:x}\n",
                r_sepc(),
                r_stval()
            );
            p.setkilled();
        }
        if p.killed() {
            kexit(-1);
        }
        // 時計割り込みならCPUを手放す
        if which_dev == 2 {
            printk!("yield\n");
            yield_cpu();
        }
        prepare_return();
        make_satp(&(*p.local.get()).pagetable)
    }
}

pub fn prepare_return() {
    let p = my_proc();

    // これから、trapで移る対象をkerneltrapからusertrapに変える。
    // 状態の一貫性を保つため、割り込みを切る
    intr_off();

    unsafe {
        let trampoline_uservec: usize = TRAMPOLINE + (USERVEC as usize - TRAMPOLINE_PHY as usize);
        w_stvec(trampoline_uservec);
    }

    // trapframeの値をセットアップする
    unsafe {
        (*(*p.local.get()).trapframe).kernel_satp = r_satp(); // kernel page table
        (*(*p.local.get()).trapframe).kernel_sp = (*p.local.get()).kstack + PG_SIZE; // kstackは下端、spは上端
        (*(*p.local.get()).trapframe).kernel_trap = usertrap as *const () as usize; // trapする先
        (*(*p.local.get()).trapframe).kernel_hartid = r_tp(); // userはtpを自由に変えられるため、cpu_idが維持されるとは限らない
    }

    // trampoline.sのsretがuser modeに戻るため使うレジスタを設定
    let mut x = r_sstatus();
    x &= !SSTATUS_SPP; // 前の特権モードはuser
    x |= SSTATUS_SPIE; // 割り込みは許可する
    w_sstatus(x);

    unsafe {
        w_sepc((*(*p.local.get()).trapframe).epc);
    }
}

// カーネルでの割り込みと例外はkernelvecを回してここに来る
#[unsafe(no_mangle)]
extern "C" fn kerneltrap() {
    let mut which_dev = 0;
    let sepc = r_sepc();
    let sstatus = r_sstatus();
    let scause = r_scause();

    if sstatus & SSTATUS_SPP == 0 {
        panic!("kerneltrap: not from supervisor mode");
    }
    if intr_get() {
        panic!("kerneltrap: interrupts enabled");
    }

    // 知らない割り込み元
    if {
        which_dev = devintr();
        which_dev
    } == 0
    {
        printk!(
            "scause=0x{:x} sepc=0x{:x} stval=0x{:x}\n",
            scause,
            r_sepc(),
            r_stval()
        );
        panic!("kerneltrap");
    }

    // タイマー割り込みならCPUを手放す
    unsafe {
        if which_dev == 2 && (*my_cpu()).proc != None {
            yield_cpu();
        }
    }

    // yield()した先でschedulerが走りCPUが手放される
    // その後、またこのプロセスが選ばれた時、この地点からユーザプロセスに戻っていく
    w_sepc(sepc);
    w_sstatus(sstatus);
}

fn clockintr() {
    if cpu_id() == 0 {
        let mut ticks = TICKS_LOCK.acquire();
        *ticks += 1;
        wakeup(&raw const TICKS_LOCK as *const ());
    }

    // 次の割り込みを要求
    // 1/10秒後程度
    w_stimecmp(r_time() + 1_000_000);
}

// 割り込みが外部割り込みか内部割り込みかを確かめ、実行する
// タイマー : 2
// 他のデバイス : 1
// 不明 : 0
fn devintr() -> u32 {
    let scause = r_scause();
    match scause {
        0x800_000_000_000_000_9 => {
            // PLICを介したsupervisor external interrupt
            // irqがどのデバイスからの割り込みかを示す
            let irq = plic_claim();
            if irq as usize == UART0_IRQ {
                uart_intr();
            } else if irq as usize == VIRTIO0_IRQ {
                // TODO virtio
            } else if irq != 0 {
                printk!("unexpected interrupt irq={}\n", irq);
            }

            // plicはそれぞれのデバイスについて、一度に最大1つの割り込みを立てる
            // plicにまた割り込みをして良いと示す
            if irq != 0 {
                plic_complete(irq);
            }
            1
        }
        0x800_000_000_000_000_5 => {
            clockintr();
            2
        }
        _ => 0,
    }
}
