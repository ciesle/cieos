pub const NCPU: usize = 8; // CPUの最大数
pub const NPROC: usize = 64; // プロセスの最大数

pub const PG_SIZE: usize = 4096;
pub const PG_SHIFT: usize = 12;

pub const NOFILE: usize = 16; // 一つのプロセスで開けるファイル数
pub const NFILE: usize = 100; // システム全体で開けるファイル数
pub const NINODE: u32 = 50; // activeなinodeの最大数
pub const NDEV: u16 = 10; // major device numberの最大数
pub const ROOT_DEV: u32 = 1; // ファイルシステムルートディスクのデバイス番号
pub const MAX_ARG: usize = 32; // exec引数の最大数
pub const MAX_OPEN_BLOCKS: u32 = 10; // 1回のファイルシステム操作が書き込める最大ブロック数
pub const LOG_BLOCKS: u32 = MAX_OPEN_BLOCKS * 3; // ディスク上における最大ログ数
pub const NBUF: u32 = MAX_OPEN_BLOCKS * 3; // ディスクブロックキャッシュのサイズ
pub const FS_SIZE: u32 = 2000; // ファイルシステムの大きさ / block
pub const MAX_PATH: usize = 128; // ファイルパス名の最大値
pub const USER_STACK: usize = 1; // user stack pages
pub const BITMAP_PER_BLOCK: u32 = BLOCK_SIZE * 8; // ブロックごとのbitmap数
pub const NBITMAP: u32 = FS_SIZE / BITMAP_PER_BLOCK + 1;
pub const ROOTINO: u32 = 1; // root i-number
pub const BLOCK_SIZE: u32 = 1024; // block size
pub const DIR_SZ: usize = 12; // ディレクトリエントリの名前の最大長
pub const NDIRECT: u32 = 12; // inodeに直接マップされるデータブロックの数
pub const NINDIRECT: u32 = BLOCK_SIZE / size_of::<u32>() as u32; // inodeから間接的にマップされるデータブロックの数
pub const MAX_FILE: u32 = NDIRECT + NINDIRECT; // 一つのinodeが使える最大ブロック数
