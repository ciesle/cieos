use super::elf::*;
use crate::{
    fs::{fs::*, log::*},
    proc::{pagetable::*, proc::*, vm::*},
    riscv::*,
    utils::printk::*,
};

use api::abi::*;
use api::param::*;
use api::vm::*;

use core::ptr::null_mut;

/// ELFのpermissionをPTEのpermissionに変換する
fn flags2perm(flags: u32) -> u32 {
    let mut perm: u32 = 0;
    if flags & 0x1 != 0 {
        perm = PteFlag::X as u32;
    }
    if flags & 0x2 != 0 {
        perm |= PteFlag::W as u32;
    }
    perm
}

/// exec()の中身
pub fn kexec(path: &[u8], argv: &[&[u8]]) -> isize {
    let p = my_proc();
    let handle = LogHandle::new();

    // 実行ファイルを開く
    let Some(mut ip) = namei(path, &handle) else {
        return -1;
    };

    let mut guard = ip.lock();
    let mut pagetable: PageTable = PageTable::new();
    let mut sz = 0;

    'start: {
        let mut elf = ElfHdr::default();
        if guard.read(
            false,
            ARange::from_mut_ref(&mut elf),
            0,
            size_of::<ElfHdr>() as u32,
        ) != Some(size_of::<ElfHdr>() as u32)
        {
            break 'start;
        }
        if elf.magic != ELF_MAGIC {
            break 'start;
        }
        let Some(mut pagetable) = p.alloc_pagetable() else {
            break 'start;
        };
        'load: {
            // プログラムをメモリにロードする
            let (mut i, mut off): (usize, u32) = (0, elf.phoff as u32);
            let mut ph = ProgHdr::default();
            while i < elf.phnum as usize {
                if guard.read(
                    false,
                    ARange::from_mut_ref(&mut ph),
                    off,
                    size_of::<ProgHdr>() as u32,
                ) != Some(size_of::<ProgHdr>() as u32)
                {
                    break 'load;
                }
                if ph.seg_type != SegType::ProgLoad as u32 {
                    i += 1;
                    off += size_of::<ProgHdr>() as u32;
                    continue;
                }
                if ph.memsz < ph.filesz {
                    break 'load;
                }
                if ph.vaddr.wrapping_add(ph.memsz) < ph.vaddr {
                    break 'load;
                }
                if ph.vaddr % PG_SIZE != 0 {
                    break 'load;
                }
                let Some(sz1) = pagetable.alloc(sz, ph.vaddr + ph.memsz, flags2perm(ph.flags))
                else {
                    break 'load;
                };
                sz = sz1;
                if !pagetable.load_seg(
                    ARange::span(VAddr(ph.vaddr), ph.filesz),
                    guard,
                    ph.off as u32,
                    ph.filesz as u32,
                ) {
                    break 'load;
                }

                i += 1;
                off += size_of::<ProgHdr>() as u32;
            }
            ip.drop(&handle);
            drop(handle);

            'stack: {
                let oldsz = p.sz;

                // 次のページ境界から数ページを確保し、最初のページをスタックガードとしてアクセス不能に、
                // 残りをユーザスタックにする
                sz = VAddr(sz).page_up().raw();
                let Some(sz1) =
                    pagetable.alloc(sz, sz + (USER_STACK + 1) * PG_SIZE, PteFlag::W as u32)
                else {
                    break 'stack;
                };
                sz = sz1;
                pagetable.clear(Page::from_raw(sz - (USER_STACK + 1) * PG_SIZE));
                let mut sp = VAddr(sz);
                let stack_base = sp - USER_STACK * PG_SIZE;

                // 引数列を新しいスタックにコピーする
                // ustack内でのアドレスを覚えておく
                let mut ustack = [const { UserArg::new() }; MAX_ARG];
                for (i, arg) in argv.iter().enumerate() {
                    sp = sp - arg.len();
                    sp = sp - sp.raw() % 16;
                    if sp < stack_base {
                        break 'stack;
                    }
                    if !copyout(&pagetable, sz, ARange::span(sp, arg.len()), arg) {
                        break 'stack;
                    }
                    ustack[i] = UserArg {
                        addr: sp.raw(),
                        len: arg.len(),
                    };
                }
                // 最後に1つヌルポインタを置く
                sp = sp - (argv.len() + 1) * size_of::<UserArg>();
                sp = sp - sp.raw() % 16;
                if sp < stack_base
                    || !copyout(
                        &pagetable,
                        sz,
                        ARange::span(sp, ((argv.len() + 1) * size_of::<UserArg>())),
                        unsafe {
                            core::slice::from_raw_parts(
                                ustack.as_ptr().cast::<u8>(),
                                ((argv.len() + 1) * size_of::<UserArg>()),
                            )
                        },
                    )
                {
                    break 'stack;
                }

                unsafe {
                    // a0とa1はuser mainへの引数を持つ。
                    // argcはシステムコールからの返り値となり、a0に入る。
                    (*(*p.local.get()).trapframe).a1 = sp.raw();

                    // デバッグ用
                    let last = path
                        .iter()
                        .rposition(|&u8| u8 == b'/')
                        .map(|v| v + 1)
                        .unwrap_or(0);
                    (*p.local.get()).name =
                        copy_name(&mut (*p.local.get()).name_inner, &path[last..]);

                    let oldpagetable =
                        core::ptr::replace(&raw mut (*p.local.get()).pagetable, pagetable);
                    (*p.local.get()).sz = sz;
                    (*(*p.local.get()).trapframe).epc = elf.entry;
                    (*(*p.local.get()).trapframe).sp = sp.raw();
                    oldpagetable.free_all(oldsz);
                }
                return argv.len() as isize;
            }
            pagetable.free_all(sz);
            return -1;
        }
        ip.drop(&handle);
        pagetable.free_all(sz);
        return -1;
    }
    ip.drop(&handle);
    return -1;
}
