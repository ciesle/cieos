// ELF実行ファイル

pub const ELF_MAGIC: u32 = 0x464C457F;

#[derive(Default)]
#[repr(C)]
pub struct ElfHdr {
    pub magic: u32,     // ELF_MAGIC
    pub elf: [u8; 12],  // エンディアン、ABIなど
    pub elf_type: u16,  // 実行ファイル、共有オブジェクトなど
    pub machine: u16,   // 対象アーキテクチャ
    pub version: u32,   // 形式のバージョン
    pub entry: usize,   // 実行開始アドレス
    pub phoff: usize,   // プログラムヘッダテーブルのオフセット
    pub shoff: usize,   // セクションヘッダテーブルのオフセット
    pub flags: u32,     // CPU固有のフラグ。圧縮命令の仕様など
    pub ehsize: u16,    // ELFヘッダ自体のサイズ
    pub phentsize: u16, // プログラムヘッダ１個のサイズ
    pub phnum: u16,     // プログラムヘッダの個数
    pub shentsize: u16, // セクションヘッダ１このサイズ
    pub shnum: u16,     // セクションヘッダの個数
    pub shstrndx: u16,  // セクション名を格納した文字列テーブルのセクション番号
}
// プログラムヘッダ：実行時にメモリへどの領域を配置するか
// セクションヘッダ：ファイル内の論理的な区分

/// プログラムヘッダ
#[derive(Default)]
#[repr(C)]
pub struct ProgHdr {
    pub seg_type: u32, // セグメントの種類。PT_LOAD, PT_DYNAMICなど
    pub flags: u32,    // セグメントのアクセス権限
    pub off: usize,    // セグメントのファイル先頭からのオフセット
    pub vaddr: usize,  // セグメントを配置する仮想アドレス
    pub paddr: usize,  // 物理アドレス。無視する
    pub filesz: usize, // セグメントのデータサイズ
    pub memsz: usize,  // メモリ上に確保するサイズ
    pub align: usize,  // セグメントのアラインメント
}

pub enum SegType {
    ProgLoad = 1,
}

pub enum ProgHdrFlag {
    Exec = 1,
    Write = 2,
    Read = 4,
}
