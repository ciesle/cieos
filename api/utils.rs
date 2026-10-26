pub fn as_bytes<T: Sized>(data: &T) -> &[u8] {
    unsafe { core::slice::from_raw_parts(core::ptr::from_ref(data) as *const u8, size_of::<T>()) }
}

pub fn as_bytes_mut<T: Sized>(data: &mut T) -> &mut [u8] {
    unsafe { core::slice::from_raw_parts_mut(core::ptr::from_mut(data) as *mut u8, size_of::<T>()) }
}
