use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};

use api::param::*;

use core::*;

#[repr(C)]
pub struct SuperBlock {
    pub magic: u32,       // FSMAGIC
    pub size: u32,        // ファイルシステムイメージの大きさ / block
    pub nblocks: u32,     // データブロックの数
    pub ninodes: u32,     // inodeの数
    pub nlog: u32,        // logブロックの数
    pub log_start: u32,   // 最初のログブロックの番号
    pub inode_start: u32, // 最初のinodeブロックの番号
    pub bmap_start: u32,  // 足所のfree mapブロックの番号
}

#[derive(PartialEq, Eq, Clone, Copy)]
#[repr(u16)]
pub enum InodeType {
    Free = 0,
    Directory = 1,
    File = 2,
    Device = 3,
}

/// ディスク上のinode構造体
#[repr(C)]
struct DInode {
    inode_type: InodeType,              // file type
    major: u16,                         // major device number
    minor: u16,                         // minor device number
    nlink: u16,                         // inodeへのリンク数
    size: u32,                          // ファイルサイズ / byte
    addrs: [u32; NDIRECT as usize + 1], // データブロックのアドレス
}
impl DInode {
    pub const fn new() -> Self {
        Self {
            inode_type: InodeType::Free,
            major: 0,
            minor: 0,
            nlink: 0,
            size: 0,
            addrs: [0; NDIRECT as usize + 1],
        }
    }
}

/// name filedはDIRSIZ文字持ち、NULLで終わらない
#[repr(C)]
pub struct DirEntry {
    pub inum: u32,
    pub name: [u8; DIR_SZ],
}
impl DirEntry {
    pub const fn new() -> Self {
        Self {
            inum: 0,
            name: [0; DIR_SZ],
        }
    }
}

// Disk layout
// [ boot block | sb block | log | inode blocks | free bit map | data blocks ]
const FSMAGIC: u32 = 0x10203040;
const NINODES: u32 = 200;
const INODE_PER_BLOCK: u32 = BLOCK_SIZE / size_of::<DInode>() as u32; // ブロックごとのinode数
const NINODE_BLOCKS: u32 = NINODES / INODE_PER_BLOCK + 1;
const NLOG: u32 = LOG_BLOCKS + 1;
const NMETA: u32 = 2 + NLOG + NINODE_BLOCKS + NBITMAP;
const NBLOCKS: u32 = FS_SIZE - NMETA;

static ZEROES: [u8; BLOCK_SIZE as usize] = [0u8; BLOCK_SIZE as usize];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    assert!(size_of::<u32>() == 4, "integers must be 4 bytes");
    let argv: Vec<String> = std::env::args().collect();
    if argv.len() < 2 {
        eprintln!("Usage: mkfs fs.img files...");
        std::process::exit(1);
    }
    assert!(BLOCK_SIZE as usize % size_of::<DInode>() == 0);
    assert!(BLOCK_SIZE as usize % size_of::<DirEntry>() == 0);

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&argv[1])?;
    let mut buf = [0u8; BLOCK_SIZE as usize];

    // 1 fs block = 1 disk sector
    // メタデータのセクター数

    let sb = SuperBlock {
        magic: FSMAGIC,
        size: FS_SIZE.to_le(),
        nblocks: NBLOCKS.to_le(),
        ninodes: NINODES.to_le(),
        nlog: NLOG.to_le(),
        log_start: 2u32.to_le(),
        inode_start: (2 + NLOG).to_le(),
        bmap_start: (2 + NLOG + NINODE_BLOCKS).to_le(),
    };
    println!(
        "nmeta {} (boot, super, log blocks {}, inode blocks {}, bitmap blocks {}) blocks {} total {}",
        NMETA, NLOG, NINODE_BLOCKS, NBITMAP, NBLOCKS, FS_SIZE
    );

    let mut free_block: u32 = NMETA; // はじめのフリーブロック
    let mut free_inode: u32 = 1;

    for i in 0..FS_SIZE {
        wsect(&mut file, i, &ZEROES);
    }

    buf[..size_of::<SuperBlock>()].copy_from_slice(api::utils::as_bytes(&sb));
    wsect(&mut file, 1, &buf);

    let root_ino = mk_dir(
        ROOTINO,
        &mut file,
        &sb,
        &mut free_block,
        &mut free_inode,
        true,
        "",
    );

    let mut hm = HashMap::<String, u32>::new();

    assert!(root_ino == ROOTINO);
    let dev_ino = mk_dir(
        root_ino,
        &mut file,
        &sb,
        &mut free_block,
        &mut free_inode,
        false,
        "dev",
    );
    hm.insert("/dev".to_string(), dev_ino);

    for arg in &argv[2..] {
        println!("{}", arg);
        let mut shortname = arg.strip_prefix("user/").unwrap_or(arg);

        let mut ino = root_ino;
        let mut place: String = "/".to_string();
        while let Some(p) = shortname.find('/') {
            let key = format!("{}/{}", place.trim_end_matches('/'), &shortname[..p]);
            let par = hm.get(&key).copied().unwrap_or_else(|| {
                let new_ino = mk_dir(
                    ino,
                    &mut file,
                    &sb,
                    &mut free_block,
                    &mut free_inode,
                    false,
                    &shortname[..p],
                );
                hm.insert(key.clone(), new_ino);
                new_ino
            });
            ino = par;
            place = key;
            shortname = &shortname[p + 1..];
        }

        let fd = OpenOptions::new().read(true).open(arg)?;
        // 先頭の_を取り除いて書き込む。
        // ビルド中に紛らわしくないように、バイナリは_rmとかの名前になっている
        let shortname = shortname.strip_prefix('_').unwrap_or(shortname);
        assert!(shortname.len() <= DIR_SZ);

        let inum = ialloc(&mut file, &sb, InodeType::File, &mut free_inode);
        let mut de = DirEntry::new();
        de.inum = inum.to_le();
        de.name[0..shortname.len()].copy_from_slice(shortname.as_bytes());
        iappend(&mut file, &sb, ino, &de, &mut free_block);
        println!("{}", fd.metadata()?.len());
        for cc in fd.bytes() {
            let cc = cc.unwrap();
            iappend(&mut file, &sb, inum, &cc, &mut free_block);
        }
    }
    /* 元のxv6では以下の処理を行っているが、
       意図が不明な上、場合によっては壊れそうな気もしたので、排除
     let mut din = rinode(&mut file, &sb, root_ino);
     let mut off = din.size.to_le();
     off = ((off / BLOCK_SIZE) + 1) * BLOCK_SIZE;
     din.size = off.to_le();
     winode(&mut file, &sb, root_ino, &din);
    */
    balloc(&mut file, &sb, free_block);
    Ok(())
}

fn mk_dir<F: Read + Write + Seek>(
    par: u32,
    file: &mut F,
    sb: &SuperBlock,
    free_block: &mut u32,
    free_inode: &mut u32,
    is_root: bool,
    name: &str,
) -> u32 {
    let ino = ialloc(file, sb, InodeType::Directory, free_inode);

    let mut de = DirEntry::new();
    de.inum = ino.to_le();
    de.name[0..1].copy_from_slice(b".");
    iappend(file, sb, ino, &de, free_block);

    de = DirEntry::new();
    de.inum = par.to_le();
    de.name[0..2].copy_from_slice(b"..");
    iappend(file, sb, ino, &de, free_block);

    if !is_root {
        let mut de = DirEntry::new();
        de.inum = ino.to_le();
        de.name[0..name.len()].copy_from_slice(name.as_bytes());
        iappend(file, sb, par, &de, free_block);
        add_link(file, sb, par);
    }

    ino
}

fn add_link<F: Read + Write + Seek>(file: &mut F, sb: &SuperBlock, inum: u32) {
    let mut din = rinode(file, sb, inum);
    din.nlink += 1;
    winode(file, sb, inum, &din);
}

/// inode iを格納するブロック番号
fn inode_block(i: u32, sb: &SuperBlock) -> u32 {
    i / INODE_PER_BLOCK + sb.inode_start
}

fn wsect<W: Write + Seek>(file: &mut W, sec: u32, buf: &[u8]) {
    assert_eq!(buf.len(), BLOCK_SIZE as usize);
    file.seek(SeekFrom::Start(sec as u64 * BLOCK_SIZE as u64))
        .expect("lseek");
    file.write_all(buf).expect("write");
}

fn winode<F: Read + Write + Seek>(file: &mut F, sb: &SuperBlock, inum: u32, din: &DInode) {
    let mut buf = [0u8; BLOCK_SIZE as usize];
    let bn = inode_block(inum, sb);
    rsect(file, bn, &mut buf);
    unsafe {
        ptr::copy_nonoverlapping(
            ptr::from_ref(din).cast::<u8>(),
            buf.as_mut_ptr()
                .add((inum % INODE_PER_BLOCK) as usize * size_of::<DInode>()),
            size_of::<DInode>(),
        );
    }
    wsect(file, bn, &buf);
}

fn rinode<R: Read + Seek>(file: &mut R, sb: &SuperBlock, inum: u32) -> DInode {
    let bn = inode_block(inum, sb);
    let mut buf = [0u8; BLOCK_SIZE as usize];
    rsect(file, bn, &mut buf);
    unsafe {
        ptr::read_unaligned(
            buf.as_ptr()
                .cast::<DInode>()
                .add((inum % INODE_PER_BLOCK) as usize),
        )
    }
}

fn rsect<R: Read + Seek>(file: &mut R, sec: u32, buf: &mut [u8]) {
    assert_eq!(
        file.seek(SeekFrom::Start((sec * BLOCK_SIZE) as u64))
            .expect("lseek"),
        (sec * BLOCK_SIZE) as u64
    );
    assert_eq!(file.read(buf).expect("read"), BLOCK_SIZE as usize);
}

fn ialloc<F: Read + Write + Seek>(
    file: &mut F,
    sb: &SuperBlock,
    inode_type: InodeType,
    free_inode: &mut u32,
) -> u32 {
    let inum = *free_inode;
    *free_inode += 1;
    let mut din = DInode::new();
    din.inode_type = inode_type;
    din.nlink = 1u16.to_le();
    din.size = 0u32.to_le();
    winode(file, sb, inum, &din);
    inum
}

fn balloc<W: Write + Seek>(file: &mut W, sb: &SuperBlock, used: u32) {
    let mut buf = [0u8; BLOCK_SIZE as usize];
    println!("balloc: first {} blocks have been allocated", used);
    assert!(used < BITMAP_PER_BLOCK);
    for i in 0..used as usize {
        buf[i / 8] = buf[i / 8] | (0x1 << (i % 8));
    }
    println!("balloc: write bitmap block at sector {}", sb.bmap_start);
    wsect(file, sb.bmap_start, &buf);
}

fn iappend<F: Write + Read + Seek, A: Sized>(
    file: &mut F,
    sb: &SuperBlock,
    inum: u32,
    p: &A,
    free_block: &mut u32,
) {
    let mut buf = [0u8; BLOCK_SIZE as usize];
    let mut din = rinode(file, sb, inum);
    let off = din.size.to_le();
    let target = api::utils::as_bytes(p);
    let mut indirect = [0u32; NINDIRECT as usize];
    for (fbn, to, from) in
        (off / BLOCK_SIZE..(off + target.len() as u32).div_ceil(BLOCK_SIZE)).map(|fbn| {
            let start = off.max(fbn * BLOCK_SIZE);
            let end = (off + target.len() as u32).min((fbn + 1) * BLOCK_SIZE);
            (
                fbn,
                start..end,
                &target[(start - off) as usize..(end - off) as usize],
            )
        })
    {
        assert!(fbn < MAX_FILE);
        let x = if fbn < NDIRECT {
            if din.addrs[fbn as usize].to_le() == 0 {
                din.addrs[fbn as usize] = *free_block;
                *free_block += 1;
            }
            din.addrs[fbn as usize]
        } else {
            if din.addrs[NDIRECT as usize].to_le() == 0 {
                din.addrs[NDIRECT as usize] = *free_block;
                *free_block += 1;
            }
            rsect(file, din.addrs[NDIRECT as usize].to_le(), unsafe {
                slice::from_raw_parts_mut(
                    indirect.as_mut_ptr() as *mut u8,
                    indirect.len() * (size_of::<u32>() / size_of::<u8>()),
                )
            });
            if indirect[fbn as usize - NDIRECT as usize] == 0 {
                indirect[(fbn - NDIRECT) as usize] = *free_block;
                *free_block += 1;
                wsect(file, din.addrs[NDIRECT as usize].to_le(), unsafe {
                    slice::from_raw_parts(
                        indirect.as_ptr() as *mut u8,
                        indirect.len() * (size_of::<u32>() / size_of::<u8>()),
                    )
                });
            }
            indirect[(fbn - NDIRECT) as usize].to_le()
        };
        rsect(file, x, &mut buf);
        buf[(to.start % BLOCK_SIZE) as usize..(to.start % BLOCK_SIZE) as usize + to.len()]
            .copy_from_slice(from);
        wsect(file, x, &buf);
    }
    din.size = din.size + size_of::<A>() as u32;
    winode(file, sb, inum, &din);
}
