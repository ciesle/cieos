use crate::proc::*;
use crate::riscv::*;
use crate::vm::*;
use core::convert::TryInto;

fn image_u16(offset: usize) -> u16 {
    u16::from_le_bytes(USER_IMAGE[offset..offset + 2].try_into().unwrap())
}

fn image_u32(offset: usize) -> u32 {
    u32::from_le_bytes(USER_IMAGE[offset..offset + 4].try_into().unwrap())
}

fn image_usize(offset: usize) -> usize {
    u64::from_le_bytes(USER_IMAGE[offset..offset + 8].try_into().unwrap()) as usize
}

// 初期プロセスの、まだユーザ領域を配置していないページテーブルに使う。
pub fn load_init_image(table: &PageTable) -> (usize, usize) {
    assert!(USER_IMAGE.len() >= 64);
    assert_eq!(&USER_IMAGE[..4], b"\x7fELF");
    assert_eq!(USER_IMAGE[4], 2); // ELF64
    assert_eq!(USER_IMAGE[5], 1); // リトルエンディアン
    assert_eq!(image_u16(18), 243); // RISC-V
    assert_eq!(image_u16(54), 56); // プログラムヘッダーのサイズ

    let entry = image_usize(24);
    let phoff = image_usize(32);
    let phnum = image_u16(56) as usize;
    let mut image_end = 0;

    for i in 0..phnum {
        let ph = phoff + i * 56;
        if image_u32(ph) != 1 {
            // PT_LOAD
            continue;
        }

        let flags = image_u32(ph + 4);
        let file_offset = image_usize(ph + 8);
        let va = image_usize(ph + 16);
        let file_size = image_usize(ph + 32);
        let mem_size = image_usize(ph + 40);
        if mem_size == 0 {
            continue;
        }
        assert!(file_size <= mem_size);
        assert_eq!(va % PG_SIZE, 0);
        assert!(va >= pgroundup(image_end));

        let end = va + mem_size;
        let mut perm = 0;
        if flags & 1 != 0 {
            // PF_X
            perm |= PTE_X;
        }
        if flags & 2 != 0 {
            // PF_W
            perm |= PTE_W;
        }
        assert_eq!(table.alloc(image_end, end, perm), end);

        // allocがページ全体をゼロ初期化するので、BSSの追加処理は不要。
        // ELFのファイル部分だけ、対応する物理ページへコピーする。
        let bytes = &USER_IMAGE[file_offset..file_offset + file_size];
        let mut copied = 0;
        while copied < bytes.len() {
            let addr = va + copied;
            let offset = addr % PG_SIZE;
            let count = (bytes.len() - copied).min(PG_SIZE - offset);
            let pte = table.walk(addr, false).expect("load_init_image");
            unsafe {
                let pa = pte2pa(*pte.as_ptr()) + offset;
                core::ptr::copy_nonoverlapping(bytes[copied..].as_ptr(), pa as *mut u8, count);
            }
            copied += count;
        }
        image_end = end;
    }

    assert!(image_end > 0 && entry < image_end);
    (entry, image_end)
}
