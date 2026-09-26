struct Context {}


struct Cpu {
    proc: *mut Proc,
    context: Context,
    noff: u32,    // push_offの深さ
    intena: bool, // push_offの前に割り込みが許可されていたか
}


struct PerCpu<T> {
    value: UnsafeCell<T>,
}

unsafe impl<T: send> Sync for PerCpu<T> {}

static CPUS: [PerCpu<Cpu>; NCPU] = [
    const {PerCpu::new(Cpu::New)};
    NCPU
];


struct TrapFrame {}

struct Proc {}



fn cpu_id() {
    r_tp()
}

fn my_cpu() -> *mut Cpu {
    CPUS[cpu_id()].value.get()
}

fn sleep_prepare(p: *mut ()) {
    let p: &mut Proc = my_proc();
    acquire(&p->lock);
}