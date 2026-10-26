#[repr(C)]
pub struct UserArg {
    pub addr: usize,
    pub len: usize,
}
impl UserArg {
    pub const fn new() -> Self {
        Self { addr: 0, len: 0 }
    }
}
