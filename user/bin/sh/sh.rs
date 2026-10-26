#![no_std]
#![no_main]

extern crate alloc;
use alloc::boxed::Box;
use api::file::*;
use api::param::MAX_ARG;
use api::param::*;
use core::error::Error;

extern crate ulib;
use ulib::print::*;
use ulib::ulib::*;
use ulib::usys::*;

const WHITE_SPACE: [char; 5] = [' ', '\t', '\r', '\n', '\x0B'];
const SYMBOLS: [char; 7] = ['<', '|', '>', '&', ';', '(', ')'];

struct ExecCmd<'a> {
    argv: [&'a str; MAX_ARG],
}
impl<'a> ExecCmd<'a> {
    pub const fn new() -> Self {
        Self {
            argv: [""; MAX_ARG],
        }
    }
}

struct RedirCmd<'a> {
    cmd: Box<Cmd<'a>>,
    file: &'a str,
    mode: FileOpenMode,
    fd: u32,
}
impl<'a> RedirCmd<'a> {
    pub const fn new(cmd: Box<Cmd<'a>>, file: &'a str, mode: FileOpenMode, fd: u32) -> Self {
        Self {
            cmd: cmd,
            file: file,
            mode: mode,
            fd: fd,
        }
    }
}

struct PipeCmd<'a> {
    left: Box<Cmd<'a>>,
    right: Box<Cmd<'a>>,
}
impl<'a> PipeCmd<'a> {
    pub const fn new(left: Box<Cmd<'a>>, right: Box<Cmd<'a>>) -> Self {
        Self {
            left: left,
            right: right,
        }
    }
}

struct ListCmd<'a> {
    left: Box<Cmd<'a>>,
    right: Box<Cmd<'a>>,
}
impl<'a> ListCmd<'a> {
    pub const fn new(left: Box<Cmd<'a>>, right: Box<Cmd<'a>>) -> Self {
        Self {
            left: left,
            right: right,
        }
    }
}

struct BackCmd<'a> {
    cmd: Box<Cmd<'a>>,
}
impl<'a> BackCmd<'a> {
    pub const fn new(cmd: Box<Cmd<'a>>) -> Self {
        Self { cmd: cmd }
    }
}

enum Cmd<'a> {
    Exec(ExecCmd<'a>),
    Redir(RedirCmd<'a>),
    Pipe(PipeCmd<'a>),
    List(ListCmd<'a>),
    Back(BackCmd<'a>),
}

fn run_cmd(cmd: Box<Cmd>) -> ! {
    match *cmd {
        Cmd::Exec(ecmd) => {
            if ecmd.argv[0].len() == 0 {
                exit(1);
            }
            let _ = exec(
                ecmd.argv[0],
                &ecmd.argv[..ecmd
                    .argv
                    .iter()
                    .position(|&arg| arg.len() == 0)
                    .unwrap_or(ecmd.argv.len())],
            );
            if !ecmd.argv[0].contains('/') {
                let mut buf = [0u8; MAX_PATH];
                buf[0..5].copy_from_slice(b"/bin/");
                buf[5..5 + ecmd.argv[0].len()].copy_from_slice(ecmd.argv[0].as_bytes());
                let _ = exec(
                    unsafe { core::str::from_utf8_unchecked(&buf[..5 + ecmd.argv[0].len()]) },
                    &ecmd.argv[..ecmd
                        .argv
                        .iter()
                        .position(|&arg| arg.len() == 0)
                        .unwrap_or(ecmd.argv.len())],
                );
            }
            fprintln!(2, "exec {} failed", ecmd.argv[0]);
        }
        Cmd::Redir(rcmd) => {
            close(rcmd.fd).unwrap();
            if open(rcmd.file, rcmd.mode).is_err() {
                fprintln!(2, "open {} failed", rcmd.file);
                exit(1);
            }
            run_cmd(rcmd.cmd);
        }
        Cmd::List(lcmd) => {
            match fork() {
                Err(()) => panic!("fork"),
                Ok(0) => run_cmd(lcmd.left),
                _ => {}
            }
            wait(0 as *mut ()).unwrap();
            run_cmd(lcmd.right);
        }
        Cmd::Pipe(pcmd) => {
            let Ok((p0, p1)) = pipe() else {
                panic!("pipe");
            };
            match fork() {
                Err(()) => panic!("fork"),
                Ok(0) => {
                    close(1).unwrap();
                    dup(p1.fd).unwrap();
                    drop(p0);
                    drop(p1);
                    run_cmd(pcmd.left);
                }
                _ => {}
            }
            match fork() {
                Err(()) => panic!("fork"),
                Ok(0) => {
                    close(0).unwrap();
                    dup(p0.fd).unwrap();
                    drop(p0);
                    drop(p1);
                    run_cmd(pcmd.right);
                }
                _ => {}
            }
            drop(p0);
            drop(p1);
            wait(0 as *mut ()).unwrap();
            wait(0 as *mut ()).unwrap();
        }
        Cmd::Back(bcmd) => match fork() {
            Err(()) => panic!("fork"),
            Ok(0) => {
                run_cmd(bcmd.cmd);
            }
            _ => {}
        },
    }
    exit(0);
}

fn get_cmd(fd: u32, buf: &mut [u8]) -> Result<&[u8], Box<dyn Error>> {
    write(fd, b"$ ")?;
    buf.fill(0);
    let ret = gets(buf);
    if ret.is_empty() || ret[0] == b'\0' {
        Err(Box::new(SomeError))
    } else {
        Ok(ret)
    }
}

#[unsafe(no_mangle)]
fn main(_: &[&str]) -> Result<(), Box<dyn Error>> {
    while let Ok(fd) = open("console", FileOpenMode::RDWR) {
        if fd >= 3 {
            close(fd)?;
            break;
        }
    }
    let mut buf = [0u8; 100];
    while let Ok(cmd) = get_cmd(2, &mut buf) {
        let Ok(mut cmd) = core::str::from_utf8(cmd) else {
            continue;
        };
        cmd = cmd.trim_start_matches([' ', '\t']);
        if cmd.starts_with('\n') {
            continue;
        }
        if cmd.starts_with("cd ") {
            match chdir(&cmd[3..].trim_end_matches(['\r', '\n'])) {
                Err(_) => {
                    fprintln!(2, "cannot cd {}", &cmd[3..].trim_end_matches(['\r', '\n']));
                }
                Ok(()) => {}
            }
        } else {
            match fork() {
                Err(()) => {
                    panic!("fork");
                }
                Ok(0) => {
                    run_cmd(parse_cmd(&mut cmd));
                }
                _ => {}
            }
            wait(0 as *mut ())?;
        }
    }
    Ok(())
}

fn parse_cmd<'a>(s: &mut &'a str) -> Box<Cmd<'a>> {
    let cmd = parse_line(s);
    *s = s.trim_start_matches(WHITE_SPACE);
    if !s.is_empty() {
        fprintln!(2, "left overs: {}", s);
        panic!("syntax");
    }
    cmd
}
fn parse_line<'a>(s: &mut &'a str) -> Box<Cmd<'a>> {
    let mut cmd = parse_pipe(s);
    while {
        *s = s.trim_start_matches(WHITE_SPACE);
        s.starts_with('&')
    } {
        get_token(s);
        cmd = Box::new(Cmd::Back(BackCmd::new(cmd)));
    }
    if {
        *s = s.trim_start_matches(WHITE_SPACE);
        s.starts_with(';')
    } {
        get_token(s);
        cmd = Box::new(Cmd::List(ListCmd::new(cmd, parse_line(s))));
    }
    cmd
}

fn parse_pipe<'a>(s: &mut &'a str) -> Box<Cmd<'a>> {
    let mut cmd = parse_exec(s);
    *s = &mut s.trim_start_matches(WHITE_SPACE);
    if s.starts_with('|') {
        get_token(s);
        cmd = Box::new(Cmd::Pipe(PipeCmd::new(cmd, parse_pipe(s))));
    }
    cmd
}

fn parse_exec<'a>(s: &mut &'a str) -> Box<Cmd<'a>> {
    *s = &mut s.trim_start_matches(WHITE_SPACE);
    if s.starts_with('(') {
        return parse_block(s);
    }

    let mut ret = Box::new(Cmd::Exec(ExecCmd::new()));
    let cmd = match ret.as_mut() {
        Cmd::Exec(cmd) => &raw mut *cmd,
        _ => unreachable!(),
    };

    let mut argc = 0;
    ret = parse_redirs(s, ret);
    // bashでは、リダイレクトはコマンドの前中後どこにでも入れられる

    while {
        *s = s.trim_start_matches(WHITE_SPACE);
        !s.starts_with(['|', ')', '&', ';'])
    } {
        let Some((tok, range)) = get_token(s) else {
            break;
        };
        assert!(tok == 'a', "syntax");
        unsafe {
            (*cmd).argv[argc] = range;
        }
        argc += 1;
        assert!(argc < MAX_ARG, "too many args");
        ret = parse_redirs(s, ret);
    }
    unsafe {
        (*cmd).argv[argc] = "";
    }
    ret
}

fn parse_block<'a>(s: &mut &'a str) -> Box<Cmd<'a>> {
    assert!(s.starts_with('('), "parse_block");
    get_token(s);
    let cmd = parse_line(s);
    assert!(s.starts_with(')'), "syntax - missing )");
    get_token(s);
    parse_redirs(s, cmd)
}

fn parse_redirs<'a>(s: &mut &'a str, mut cmd: Box<Cmd<'a>>) -> Box<Cmd<'a>> {
    while {
        *s = s.trim_start_matches(WHITE_SPACE);
        s.starts_with(['<', '>'])
    } {
        let (type1, _) = get_token(s).unwrap();
        let Some(('a', token)) = get_token(s) else {
            panic!("missing file for redirection");
        };
        match type1 {
            '<' => {
                cmd = Box::new(Cmd::Redir(RedirCmd::new(
                    cmd,
                    token,
                    FileOpenMode::RD_ONLY,
                    0,
                )))
            }
            '>' => {
                cmd = Box::new(Cmd::Redir(RedirCmd::new(
                    cmd,
                    token,
                    FileOpenMode::WR_ONLY | FileOpenMode::CREATE | FileOpenMode::TRUNC,
                    1,
                )))
            }
            '+' => {
                cmd = Box::new(Cmd::Redir(RedirCmd::new(
                    cmd,
                    token,
                    FileOpenMode::WR_ONLY | FileOpenMode::CREATE,
                    1,
                )))
            }
            _ => unreachable!(),
        }
    }
    cmd
}

fn get_token<'a>(s: &mut &'a str) -> Option<(char, &'a str)> {
    *s = s.trim_start_matches(WHITE_SPACE);
    if s.is_empty() {
        return None;
    }
    let mut retc;
    let mut retr: &'a str;
    match s.chars().next()? {
        '|' | '(' | ')' | ';' | '&' | '<' => {
            let sz = s.chars().next().map(|c| c.len_utf8())?;
            retc = s.chars().next()?;
            retr = &s[..sz];
            *s = &s[sz..];
        }
        '>' => {
            let sz = s.chars().next().map(|c| c.len_utf8())?;
            retc = '>';
            retr = &s[..sz];
            *s = &s[sz..];
            if !s.is_empty() && s.chars().next()? == '>' {
                let sz = s.chars().next().map(|c| c.len_utf8())?;
                retc = '+';
                retr = &s[..sz];
                *s = &s[sz..];
            }
        }
        _ => {
            retc = 'a';
            let sz = s
                .find(|c| WHITE_SPACE.contains(&c) || SYMBOLS.contains(&c))
                .unwrap_or(s.len());
            retr = &s[..sz];
            *s = &s[sz..];
        }
    }
    *s = s.trim_start_matches(WHITE_SPACE);
    Some((retc, retr))
}
