// file-system system calls
// ユーザを信用しないため、ほとんどの引数を確認した後、file.rsやfs.rsを呼び出す

use super::{exec::*, syscall::*};
use crate::{
    fs::{file::*, fs::*, inode::*, log::*, pipe::*},
    proc::{kalloc::*, proc::*, vm::*},
};

use core::ptr::NonNull;
use core::ptr::null_mut;

use api::file::*;
use api::param::*;
use api::vm::*;

/// n番目のword sizeのsystem call引数をファイル記述子として取得
/// 記述子とファイルハンドラを返す
fn argfd(n: u32) -> Option<(u32, *mut FileHandle)> {
    let fd = argraw(n) as u32;
    let p = my_proc();
    unsafe {
        (*p.local.get())
            .ofile
            .get_mut(fd as usize)
            .and_then(Option::as_mut)
            .map(|handle| (fd, handle as *mut FileHandle))
    }
}

/// 与えられたファイルに対してファイル記述子を割り当てる
/// 成功したら、呼び出し者からファイル参照の所有権を引き継ぐ
fn fd_alloc(f: FileHandle) -> Option<u32> {
    let p = my_proc();
    unsafe {
        (0..NOFILE).position(|fd| p.ofile[fd].is_none()).map(|fd| {
            (*p.local.get()).ofile[fd] = Some(f);
            fd as u32
        })
    }
}

pub fn sys_dup() -> isize {
    unsafe {
        let Some((_, f)) = argfd(0) else {
            return -1;
        };
        fd_alloc((*f).clone()).map_or(-1, |fd| fd as isize)
    }
}

pub fn sys_read() -> isize {
    let p = argaddr(1);
    let n = argraw(2) as u32;
    let Some((_, f)) = argfd(0) else {
        return -1;
    };
    unsafe {
        (*f).read(ARange::span(p, n as usize))
            .map_or(-1, |x| x as isize)
    }
}

pub fn sys_write() -> isize {
    let p = argaddr(1);
    let n = argraw(2) as u32;
    let Some((_, f)) = argfd(0) else {
        return -1;
    };
    unsafe {
        (*f).write(ARange::span(p, n as usize))
            .map_or(-1, |x| x as isize)
    }
}

pub fn sys_close() -> isize {
    let Some((fd, _)) = argfd(0) else {
        return -1;
    };
    unsafe {
        (*my_proc().local.get()).ofile[fd as usize] = None;
        0
    }
}

/// fのメタデータを取得し、addrにコピーする
pub fn sys_fstat() -> isize {
    let p = my_proc();
    let addr = argaddr(1); // 結果を書き込むアドレス

    let Some((_, f)) = argfd(0) else {
        return -1;
    };
    unsafe {
        if let Some(r) = (*f).state() {
            if copyout(
                &p.pagetable,
                p.sz,
                ARange::span(addr, size_of::<FileState>()),
                api::utils::as_bytes(&r),
            ) {
                0
            } else {
                -1
            }
        } else {
            -1
        }
    }
}

/// oldと同じinodeを参照するリンクを作成する
pub fn sys_link() -> isize {
    let mut old = [0_u8; MAX_PATH];
    let mut new = [0_u8; MAX_PATH];
    if argstr(0, &mut old).is_none() || argstr(1, &mut new).is_none() {
        return -1;
    }

    let handle = LogHandle::new();
    let Some(mut ip) = namei(&old, &handle) else {
        return -1;
    };

    {
        let mut guard = ip.lock();
        if guard.inode_type == InodeType::Directory {
            ip.drop(&handle);
            return -1;
        }

        guard.nlink += 1;
        guard.update(&handle);
        ip.unlock();
    }

    let linked = if let Some((mut dp, name)) = nameiparent(&new, &handle) {
        let mut guard = dp.lock();
        let linked = guard.dev == ip.dev && dir_link(name, ip.inum, &mut guard, &handle);
        dp.drop(&handle);
        linked
    } else {
        false
    };
    if linked {
        ip.drop(&handle);
        0
    } else {
        let mut guard = ip.lock();
        guard.nlink -= 1;
        guard.update(&handle);
        ip.drop(&handle);
        -1
    }
}

/// ディレクトリは空か？
fn is_dir_empty(guard: &mut InodeGuard) -> bool {
    let mut de: DirEntry = DirEntry::new();
    (2 * size_of::<DirEntry>()..guard.size as usize)
        .step_by(size_of::<DirEntry>())
        .all(|off| {
            assert!(
                guard.read(
                    false,
                    ARange::from_mut_ref(&mut de),
                    off as u32,
                    size_of::<DirEntry>() as u32
                ) == Some(size_of::<DirEntry>() as u32),
                "is_dir_empty: inode.read"
            );
            de.inum == 0
        })
}

pub fn sys_unlink() -> isize {
    let mut path = [0u8; MAX_PATH];
    if argstr(0, &mut path).is_none() {
        return -1;
    }
    let handle = LogHandle::new();
    let Some((mut dp, name)) = nameiparent(&path, &handle) else {
        return -1;
    };
    let mut gdp = dp.lock();
    'bad: {
        if name == b"." || name == b".." {
            break 'bad;
        }
        let Some((mut ip, off)) = dir_lookup(name, &mut gdp, &handle) else {
            break 'bad;
        };
        let mut gip = ip.lock();
        assert!(gip.nlink >= 1, "unlink: nlink < 1");
        if gip.inode_type == InodeType::Directory && !is_dir_empty(&mut gip) {
            ip.drop(&handle);
            break 'bad;
        }
        let mut de = DirEntry::new();
        if gdp.write(
            false,
            ARange::from_ref(&de),
            off,
            size_of::<DirEntry>() as u32,
            &handle,
        ) != Some(size_of::<DirEntry>() as u32)
        {
            panic!("unlink: writei");
        }
        if gip.inode_type == InodeType::Directory {
            gdp.nlink -= 1;
            gdp.update(&handle);
        }
        dp.drop(&handle);
        gip.nlink -= 1;
        gip.update(&handle);
        ip.drop(&handle);
        return 0;
    }
    dp.drop(&handle);
    return -1;
}

fn create(
    path: &[u8],
    inode_type: InodeType,
    major: u16,
    minor: u16,
    handle: &LogHandle,
) -> Option<InodeHandle> {
    let Some((mut dp, name)) = nameiparent(path, handle) else {
        return None;
    };
    let mut gdp = dp.lock();
    if gdp.nlink == 0 {
        dp.drop(handle);
        return None;
    }
    if let Some((mut ip, _)) = dir_lookup(name, &mut gdp, handle) {
        dp.drop(handle);
        let mut gip = ip.lock();
        if inode_type == InodeType::File
            && (gip.inode_type == InodeType::File || gip.inode_type == InodeType::Device)
        {
            return Some(ip);
        }
        ip.drop(handle);
        return None;
    }
    let Some(mut ip) = InodeHandle::alloc(gdp.dev, inode_type, handle) else {
        dp.drop(handle);
        return None;
    };
    let mut gip = ip.lock();
    gip.major = major;
    gip.minor = minor;
    gip.nlink = 1;
    gip.update(handle);

    'fail: {
        // .と..を作る
        if inode_type == InodeType::Directory {
            if !dir_link(b".", gip.inum, &mut gip, handle)
                || !dir_link(b"..", gdp.inum, &mut gip, handle)
            {
                break 'fail;
            }
        }
        if !dir_link(name, gip.inum, &mut gdp, handle) {
            break 'fail;
        }
        if inode_type == InodeType::Directory {
            gdp.nlink += 1;
            gdp.update(handle);
        }
        dp.drop(handle);
        return Some(ip);
    }
    gip.nlink = 0;
    gip.update(handle);
    ip.drop(handle);
    dp.drop(handle);
    None
}

pub fn sys_open() -> isize {
    let omode = FileOpenMode(argraw(1) as u32);
    let mut path = [0u8; MAX_PATH];

    if argstr(0, &mut path).is_none() {
        return -1;
    }

    let handle = LogHandle::new();
    let mut ip = if omode & FileOpenMode::CREATE {
        match create(&path, InodeType::File, 0, 0, &handle) {
            Some(mut ip) => ip,
            None => return -1,
        }
    } else {
        match namei(&path, &handle) {
            Some(mut ip) => ip,
            None => return -1,
        }
    };
    let mut guard = if omode & FileOpenMode::CREATE {
        ip.acquire_without_lock()
    } else {
        ip.lock()
    };
    if guard.inode_type == InodeType::Directory && omode != FileOpenMode::RD_ONLY {
        ip.drop(&handle);
        return -1;
    }
    if guard.inode_type == InodeType::Device && guard.major >= NDEV {
        ip.drop(&handle);
        return -1;
    }

    let Some(fd) = FileHandle::new().and_then(|mut f| -> Option<u32> { fd_alloc(f) }) else {
        ip.drop(&handle);
        return -1;
    };

    let mut f = unsafe {
        (*my_proc().local.get()).ofile[fd as usize]
            .as_mut()
            .unwrap()
    };
    if guard.inode_type == InodeType::Device {
        f.file_type = FileType::FdDevice;
        f.major = guard.major;
    } else {
        f.file_type = FileType::FdInode;
        f.off = 0;
    }
    f.readable = !(omode & FileOpenMode::WR_ONLY);
    f.writable = (omode & FileOpenMode::WR_ONLY) || (omode & FileOpenMode::RDWR);
    if (omode & FileOpenMode::TRUNC) && guard.inode_type == InodeType::File {
        guard.trunc(&handle);
    }
    f.ip = ip;
    f.ip.unlock();
    fd as isize
}

pub fn sys_mkdir() -> isize {
    let handle = LogHandle::new();
    let mut path = [0u8; MAX_PATH];
    let Some(mut ip) = argstr(0, &mut path)
        .is_some()
        .then(|| create(&path, InodeType::Directory, 0, 0, &handle))
        .flatten()
    else {
        return -1;
    };
    ip.drop(&handle);
    0
}

pub fn sys_mknod() -> isize {
    let handle = LogHandle::new();
    let major = argraw(1) as u16;
    let minor = argraw(2) as u16;
    let mut path = [0u8; MAX_PATH];
    let Some(mut ip) = argstr(0, &mut path)
        .is_some()
        .then(|| create(&path, InodeType::Device, major, minor, &handle))
        .flatten()
    else {
        return -1;
    };
    ip.drop(&handle);
    0
}

pub fn sys_chdir() -> isize {
    let p = my_proc();
    let mut handle = LogHandle::new();
    let mut path = [0u8; MAX_PATH];
    let Some(mut ip) = argstr(0, &mut path)
        .is_some()
        .then(|| namei(&path, &handle))
        .flatten()
    else {
        return -1;
    };
    let mut guard = ip.lock();
    if guard.inode_type != InodeType::Directory {
        ip.drop(&handle);
        return -1;
    }
    ip.unlock();
    unsafe {
        ip = core::ptr::replace(&raw mut (*p.local.get()).cwd, ip);
        ip.drop(&handle);
        drop(handle);
    }
    0
}

pub fn sys_exec() -> isize {
    //path: path
    // uargv: 引数のポインタ列(0-ending)が収まっている場所のポインタ
    let uargv = argaddr(1);
    let mut path = [0u8; MAX_PATH];
    if argstr(0, &mut path).is_none() {
        return -1;
    }
    let mut argv: [&[u8]; MAX_ARG] = [&[]; MAX_ARG];
    let mut raw: [Option<Frame>; MAX_ARG] = [None; MAX_ARG];
    let mut i = 0;
    if !loop {
        if i >= MAX_ARG {
            break false;
        }
        let Some(uarg) = fetch_addr(uargv + size_of::<usize>() * i) else {
            break false;
        };
        if uarg == VAddr(0) {
            argv[i] = &[];
            break true;
        }
        unsafe {
            let ip = match kalloc() {
                Some(ip) => ip,
                None => break false,
            };
            raw[i] = Some(ip);
            match fetch_str(
                uarg,
                core::slice::from_raw_parts_mut(ip.as_mut_ptr(), PG_SIZE),
            ) {
                Some(len) => {
                    argv[i] = core::slice::from_raw_parts(ip.as_ptr(), len);
                }
                None => break false,
            }
        }
        i += 1;
    } {
        for i in (0..MAX_ARG) {
            match raw[i] {
                Some(ip) => kfree(ip),
                None => break,
            }
        }
        return -1;
    }

    let ret = kexec(
        &path[..path.iter().position(|&x| x == 0).unwrap_or(path.len())],
        &argv[0..i],
    );

    for i in (0..MAX_ARG) {
        match raw[i] {
            Some(ip) => kfree(ip),
            None => break,
        }
    }
    ret
}

pub fn sys_pipe() -> isize {
    let p = my_proc();
    let fdarray = argaddr(0);
    let Some((rf, wf)) = Pipe::alloc() else {
        return -1;
    };
    let (fd0, fd1) = (fd_alloc(rf), fd_alloc(wf));
    let (Some(fd0), Some(fd1)) = (fd0, fd1) else {
        for fd in [fd0, fd1].into_iter().flatten() {
            unsafe {
                (*p.local.get()).ofile[fd as usize] = None;
            }
        }
        return -1;
    };
    if !copyout(
        &p.pagetable,
        p.sz,
        ARange::span(fdarray, size_of::<u32>()),
        &fd0.to_ne_bytes(),
    ) || !copyout(
        &p.pagetable,
        p.sz,
        ARange::span(fdarray + size_of::<u32>(), size_of::<u32>()),
        &fd1.to_ne_bytes(),
    ) {
        unsafe {
            (*p.local.get()).ofile[fd0 as usize] = None;
            (*p.local.get()).ofile[fd1 as usize] = None;
        }
        return -1;
    }
    0
}
