use super::uart::*;
use crate::{
    fs::file::*,
    proc::{proc::*, spinlock::*},
};
use core::mem;

use api::param::*;
use api::vm::*;

pub enum ConsoleChar {
    Char(u8),
    BackSpace,
}
impl From<u8> for ConsoleChar {
    fn from(c: u8) -> Self {
        Self::Char(c)
    }
}

const CTRL_H: u8 = b'H' - b'@';
const CTRL_U: u8 = b'U' - b'@';
const CTRL_D: u8 = b'D' - b'@';
const CTRL_P: u8 = b'P' - b'@';

/// uartに1文字送る。
/// interruptやsleepを使わず、interrupt内から使えるように
pub fn consputc(c: impl Into<ConsoleChar>) {
    match c.into() {
        ConsoleChar::BackSpace => {
            uart_putc_sync(b'\x08');
            uart_putc_sync(b' ');
            uart_putc_sync(b'\x08');
        }
        ConsoleChar::Char(c) => {
            uart_putc_sync(c);
        }
    }
}

const INPUT_BUF_SIZE: usize = 128;
struct Cons {
    buf: [u8; INPUT_BUF_SIZE],
    r: usize,
    w: usize,
    e: usize,
}
static CONS_LOCK: SpinLock<Cons> = SpinLock::new(
    Cons {
        buf: [0; INPUT_BUF_SIZE],
        r: 0, // 一番先端 index
        w: 0, // 確定 index
        e: 0, // 編集した index
    },
    "cons",
);

/// userのwrite system callがここに来る。sleepも使う
pub fn console_write(user_src: bool, src: ARange<VAddr>) -> Option<usize> {
    let mut buf: [u8; 32] = [0; 32];
    let mut i: usize = 0;

    while i < src.len() {
        let mut nn = core::cmp::min(src.len() - i, mem::size_of_val(&buf));
        if !either_copyin(&mut buf[..nn], user_src, ARange::span(src.start + i, nn)) {
            break;
        }
        uart_write(&buf, nn);
        i += nn;
    }
    Some(i)
}

/// userのread()はこれを呼び出す
/// input line全体をdstにコピーする
/// user_dstはコピー先がユーザ空間かカーネル空間かを示す
/// 読んだ文字数を返す
pub fn console_read(user_dst: bool, mut dst: ARange<VAddr>) -> Option<usize> {
    let target = dst.len();
    let mut cons = CONS_LOCK.acquire();
    while dst.len() > 0 {
        // interrupt handlerが文字を入れるまで待つ
        while cons.r == cons.w {
            if my_proc().killed() {
                return None;
            }
            sleep_prepare(&raw const cons.r as *const ());
            drop(cons);
            sleep();
            cons = CONS_LOCK.acquire();
        }

        let c = cons.buf[cons.r % INPUT_BUF_SIZE];
        cons.r += 1;
        // end-of-file
        if c == CTRL_D {
            if dst.len() < target {
                // 次呼び出されたときに 0-byte の結果が得られたと
                // わかるために、^Dを保存しておく。
                // この方法でxv6は^Dを通知する
                cons.r -= 1;
            }
            break;
        }
        let mut cbuf = c;
        if !either_copyout(
            user_dst,
            ARange::span(dst.start, 1),
            &mut cbuf.to_ne_bytes(),
        ) {
            break;
        }
        dst.start = dst.start + 1;

        if c == b'\n' {
            // 行全体を得られたので、戻る
            break;
        }
    }
    Some(target - dst.len())
}

/// console 読み込み割り込み handler
/// uartintr()が文字ごとにこれを呼び出す
/// eraseやkillを行い、cons.bufに入れ、行全体が届いていたらconsole_read()を呼び出す
pub fn console_intr(c: u8) {
    let mut cons = CONS_LOCK.acquire();
    match c {
        c if c == CTRL_P => procdump(), // Print process list
        CTRL_U => {
            // kill line
            while cons.e != cons.w && cons.buf[(cons.e - 1) % INPUT_BUF_SIZE] != b'\n' {
                cons.e -= 1;
                consputc(ConsoleChar::BackSpace);
            }
        }
        CTRL_H | b'\x7f' => {
            if cons.e != cons.w {
                cons.e -= 1;
                consputc(ConsoleChar::BackSpace);
            }
        }
        mut c => {
            if c != 0 && cons.e - cons.r < INPUT_BUF_SIZE {
                c = if c == b'\r' { b'\n' } else { c };

                // エコーバックする
                consputc(c);

                // console_readのために保存
                let i = cons.e % INPUT_BUF_SIZE;
                cons.buf[i] = c;
                cons.e += 1;

                // 行全体化ファイルの終わりが来たらconsole_readを起こす
                if c == b'\n' || c == CTRL_D || cons.e - cons.r == INPUT_BUF_SIZE {
                    cons.w = cons.e;
                    wakeup(&raw const cons.r as *const ());
                }
            }
        }
    };
}

pub fn console_init() {
    uart_init();
    DEV_SW[CONSOLE].read.set(console_read);
    DEV_SW[CONSOLE].write.set(console_write);
}
