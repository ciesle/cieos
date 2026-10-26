use core::ptr::null_mut;

use crate::usys::*;
use api::param::*;

struct Header {
    ptr: *mut Header,
    size: usize,
}
impl Header {
    pub const fn new() -> Self {
        Self {
            ptr: null_mut(),
            size: 0,
        }
    }
}

static mut BASE: Header = Header::new();
static mut FREEP: *mut Header = null_mut();

fn free(ap: *mut ()) {
    unsafe {
        let bp = (ap as *mut Header).sub(1);
        let mut p = FREEP;
        while !(p < bp && bp < (*p).ptr) {
            if p >= (*p).ptr && (bp > p || bp < (*p).ptr) {
                break;
            }
            p = (*p).ptr;
        }

        if bp.add((*bp).size) == (*p).ptr {
            (*bp).size += (*(*p).ptr).size;
            (*bp).ptr = (*(*p).ptr).ptr;
        } else {
            (*bp).ptr = (*p).ptr;
        }

        if p.add((*p).size) == bp {
            (*p).size += (*bp).size;
            (*p).ptr = (*bp).ptr;
        } else {
            (*p).ptr = bp;
        }
        FREEP = p;
    }
}

fn more_core(mut nu: usize) -> Option<*mut Header> {
    if nu < PG_SIZE {
        nu = PG_SIZE;
    }
    if nu * size_of::<Header>() > i32::MAX as usize {
        return None;
    }
    let Ok(p) = sbrk((nu * size_of::<Header>()) as isize).map(|x| x as *mut Header) else {
        return None;
    };
    if p as usize == usize::MAX {
        return None;
    }
    unsafe {
        (*p).size = nu;
        free(p.add(1) as *mut ());
        Some(FREEP)
    }
}

fn malloc(nbytes: usize) -> Option<*mut ()> {
    let nunits = (nbytes + size_of::<Header>() - 1) / size_of::<Header>() + 1;
    let mut prevp: *mut Header;
    unsafe {
        prevp = FREEP;
        if prevp == null_mut() {
            BASE.ptr = &raw mut BASE;
            FREEP = &raw mut BASE;
            prevp = &raw mut BASE;
            BASE.size = 0;
        }

        let mut p = (*prevp).ptr;
        loop {
            if (*p).size >= nunits {
                if (*p).size == nunits {
                    (*prevp).ptr = (*p).ptr;
                } else {
                    (*p).size -= nunits;
                    p = p.add((*p).size);
                    (*p).size = nunits;
                }
                FREEP = prevp;
                return Some(p.add(1) as *mut ());
            }
            prevp = p;
            p = (*p).ptr;
            if p == FREEP {
                match more_core(nunits) {
                    None => return None,
                    Some(ptr) => p = ptr,
                }
            }
        }
    }
}

use core::alloc::{GlobalAlloc, Layout};

struct UserAllocator;

#[global_allocator]
static ALLOCATOR: UserAllocator = UserAllocator;
unsafe impl GlobalAlloc for UserAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.align() > 8 {
            return null_mut();
        }
        match malloc(layout.size()) {
            None => null_mut(),
            Some(p) => p as *mut u8,
        }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        free(ptr as *mut ());
    }
}
