// シンプルなロギングシステム
//
// 一つのトランザクションは、複数のFS system callを含む
// アクティブなシステムコールがないときにコミットされる。
// そのため、コミットが終了していないシステムコールの更新を書き込むかを心配しないで良い
//
// システムコールは、begin_opとend_opを呼んで開始と終了を示す
// begin_opは基本的にカウントを増やすだけだが、
// これを実行してログが足りなくなりそうならスリープする
//
// ログはディスクブロックを含む物理redoログである
// header block（ブロックAの番号、ブロックBの番号、・・・）
// ブロック A
// ブロック B
// ...
//
// ログ追加は同期的

use super::{buf::*, fs::*};
use crate::{
    proc::{proc::*, spinlock::*},
    utils::printk::*,
};
use core::cell::OnceCell;
use core::ops::{Deref, DerefMut};

use api::param::*;
use api::vm::*;

/// ヘッダーブロックの内容
/// ディスク上に保存するためと、コミット前にログ対象ディスクブロックを把握するため
#[repr(C)]
#[derive(Clone, Copy)]
struct LogHeader {
    n: u32,
    block: [u32; LOG_BLOCKS as usize],
}
struct LogState {
    outstanding: u32, // いくつのFS syscallが動いているか
    comitting: bool,  // 他にコミット中のCPUがいたりするか
    ncommit: usize,
    lh: LogHeader, // アクセスするには、ロックされているか、コミット中か、初期化中かが必要
}
struct LogPublic {
    start: u32,
    dev: u32,
}

struct Log {
    public: OnceCell<LogPublic>,
    lk: SpinLock<LogState>,
}
impl Deref for Log {
    type Target = LogPublic;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.public.get().unwrap() }
    }
}
unsafe impl Sync for Log {}
static LOG: Log = Log {
    public: OnceCell::new(),
    lk: SpinLock::new(
        LogState {
            outstanding: 0,
            comitting: false,
            ncommit: 0,
            lh: LogHeader {
                n: 0,
                block: [0; LOG_BLOCKS as usize],
            },
        },
        "log",
    ),
};

pub fn init_log(dev: u32, sb: *const SuperBlock) {
    assert!(
        (size_of::<LogHeader>() as u32) < BLOCK_SIZE,
        "initlog: too big logheader"
    );
    unsafe {
        LOG.public.set(LogPublic {
            start: (&*sb).log_start,
            dev: dev,
        });
    }
    let mut state = LOG.lk.acquire();
    let mut commit = CommitHandle::new_recover(&mut state);
    drop(state);
    commit.recover_from_log();
}

/// LogHeaderへのアクセスが可能となるタイミングは、
/// - ログのロックを取っているとき
/// - commiting = true のとき
/// - 起動直後で、commitもロックもないと保証できるとき
/// である。つまり、LogHeaderはロックがなくてもアクセスできる必要があるが、
/// ロック外に露出させたくなかったため、CommitHandleに格納することにした
/// ただ、これらの関数はLogHandleの最後でしか呼ばれないため、
struct CommitHandle {
    lh: *mut LogHeader,
    recovering: bool,
}
impl Deref for CommitHandle {
    type Target = LogHeader;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lh }
    }
}
impl DerefMut for CommitHandle {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lh }
    }
}
impl CommitHandle {
    fn new(state: &mut Guard<'static, LogState, SpinLock<LogState>>) -> Self {
        assert!(!state.comitting, "log.comitting");
        state.comitting = true;
        Self {
            lh: unsafe { &raw mut state.lh },
            recovering: false,
        }
    }
    fn new_recover(state: &mut Guard<'static, LogState, SpinLock<LogState>>) -> Self {
        Self {
            lh: unsafe { &raw mut state.lh },
            recovering: true,
        }
    }
    /// コミットしたブロックをログから割り当てられた位置に戻す
    fn install_trans(&mut self) {
        for tail in 0..self.n {
            if self.recovering {
                printk!(
                    "recovering tail {} dest {}\n",
                    tail,
                    self.block[tail as usize]
                );
            }
            unsafe {
                let (lbuf, mut lguard) = bread(LOG.dev, LOG.start + tail + 1); // read log  block
                let (dbuf, mut dguard) = bread(LOG.dev, self.block[tail as usize]); // read dst
                core::ptr::copy_nonoverlapping(
                    lguard.data.as_ptr(),
                    dguard.data.as_mut_ptr(),
                    BLOCK_SIZE as usize,
                );
                bwrite(dbuf, &mut dguard);
                if !self.recovering {
                    bunpin(dbuf);
                }
            }
        }
    }

    /// ディスク上のlog headerからメモリ上のlog headerに読み込む
    fn read_head(&mut self) {
        unsafe {
            let (buf, guard) = bread(LOG.dev, LOG.start);
            **self = (guard.data.as_ptr() as *const LogHeader).read_unaligned();
        }
    }

    /// メモリ上のlog headerをディスクに書き込む
    /// トランザクションが真にコミットされるタイミング
    fn write_head(&mut self) {
        unsafe {
            let (buf, mut guard) = bread(LOG.dev, LOG.start);
            (guard.data.as_mut_ptr() as *mut LogHeader).write(**self);
            bwrite(buf, &mut guard);
        }
    }

    fn recover_from_log(&mut self) {
        self.read_head();
        self.install_trans(); // コミットされたデータをログからディスクへ転送
        self.n = 0;
        self.write_head();
    }

    /// 本来のブロックのキャッシュに書かれている内容を、
    /// ディスク上とメモリ上のログブロックの該当の場所に書きこむ
    fn write_log(&mut self) {
        for tail in 0..self.n {
            let (to, mut to_guard) = bread(LOG.dev, LOG.start + tail + 1);
            let (from, mut from_guard) = bread(LOG.dev, self.block[tail as usize]);
            unsafe {
                to_guard
                    .data
                    .as_mut_ptr()
                    .copy_from_nonoverlapping(from_guard.data.as_ptr(), BLOCK_SIZE as usize);
            }
            bwrite(to, &mut to_guard);
        }
    }

    pub fn commit(&mut self) {
        if self.n > 0 {
            self.write_log(); // 変更したブロックをキャッシュからログに書き込む
            self.write_head(); // ヘッダをディスクに移す
            self.install_trans(); // ログ領域を元の場所に戻す
            self.n = 0;
            self.write_head(); // トランザクションを消す
        }
    }
}
impl Drop for CommitHandle {
    fn drop(&mut self) {
        let mut state = LOG.lk.acquire();
        state.comitting = false;
        state.ncommit += 1;
        wakeup(&raw const LOG as *const ());
    }
}

pub struct LogHandle {}
impl LogHandle {
    /// 毎度のFS systemcallの最初に呼ばれる
    pub fn new() -> Self {
        let mut state = LOG.lk.acquire();
        loop {
            if state.comitting
                || state.lh.n + (state.outstanding + 1) * MAX_OPEN_BLOCKS > LOG_BLOCKS
            {
                sleep_prepare((&raw const LOG).cast());
                drop(state);
                sleep();
                state = LOG.lk.acquire();
            } else {
                state.outstanding += 1;
                break LogHandle {};
            }
        }
    }
    /*
    呼び出し元はbuf.dataを変更しおわった。
    ブロック番号を記録し、refcntを増すことでキャッシュにピンする
    commitとwrite_locがディスクに書き込む

    log_writeはbwriteを置き換える。
    この関数はLohHeaderに更新したBufを追加するだけで、実際の書き込みは
    LogHandle.drop内でcommitが行う
     */
    pub fn write(&self, b: *mut Buf) {
        let mut state = LOG.lk.acquire();
        assert!(state.lh.n < LOG_BLOCKS, "too big a transaction");
        assert!(state.outstanding >= 1, "log_write outside of transaction");
        let i = match (0..state.lh.n)
            .find(|&i| unsafe { state.lh.block[i as usize] == (*b).blockno })
        {
            Some(v) => v,
            None => state.lh.n,
        };
        // 新しいブロックをlogに足す場合
        if i == state.lh.n {
            unsafe {
                state.lh.block[i as usize] = (*b).blockno;
            }
            bpin(b);
            state.lh.n += 1;
        }
    }
}
impl Drop for LogHandle {
    /// 毎度のシステムコールの最後に呼ばれる。
    /// 最後のoutstandingなら、commitする
    fn drop(&mut self) {
        let commit = {
            let mut state = LOG.lk.acquire();
            state.outstanding -= 1;
            if state.outstanding == 0 {
                // コミット開始
                Some(CommitHandle::new(&mut state))
            } else {
                wakeup(&raw const LOG as *const ());
                None
            }
        };
        if let Some(mut commit) = commit {
            // コミットを行うとき、spinlockを持っているのは許されない
            commit.commit();
            // コミット終了（RAII）
        }
    }
}

pub fn sys_sync() -> isize {
    let mut log = LOG.lk.acquire();
    if log.comitting || log.outstanding > 0 {
        let n = log.ncommit + 1;
        while log.ncommit < n {
            sleep_prepare(&raw const LOG as *const ());
            drop(log);
            sleep();
            log = LOG.lk.acquire();
        }
    }
    0
}
