//! 原本: xv6-riscv/user/usertests.c のファイルシステム検査。
#![allow(unsafe_op_in_unsafe_fn)]
use super::support::*;
use api::file::DirEntry;
use ulib::print::*;

pub unsafe fn truncate1(_: &str) {
    let mut b = [0; 32];
    unlink("truncfile");
    let f1 = fd("truncfile", CREATE | WRONLY | TRUNC);
    write(f1, b"abcd");
    close(f1);
    let f2 = fd("truncfile", 0);
    need(read(f2, &mut b) == 4, "initial read != 4");
    let f1 = fd("truncfile", WRONLY | TRUNC);
    let f3 = fd("truncfile", 0);
    need(read(f3, &mut b) == 0, "new fd not truncated");
    need(read(f2, &mut b) == 0, "old fd not truncated");
    write(f1, b"abcdef");
    need(read(f3, &mut b) == 6, "new fd read != 6");
    need(read(f2, &mut b) == 2, "old offset not preserved");
    unlink("truncfile");
    close(f1);
    close(f2);
    close(f3);
}
pub unsafe fn truncate2(_: &str) {
    unlink("truncfile");
    let f1 = fd("truncfile", CREATE | TRUNC | WRONLY);
    write(f1, b"abcd");
    let f2 = fd("truncfile", TRUNC | WRONLY);
    need(
        write(f1, b"x") == -1,
        "write beyond truncated end succeeded",
    );
    unlink("truncfile");
    close(f1);
    close(f2);
}
pub unsafe fn truncate3(_: &str) {
    close(fd("truncfile", CREATE | TRUNC | WRONLY));
    if spawn() == 0 {
        for _ in 0..100 {
            let f = fd("truncfile", WRONLY);
            need(write(f, b"1234567890") == 10, "write != 10");
            close(f);
            let f = fd("truncfile", 0);
            read(f, &mut [0; 32]);
            close(f);
        }
        exit(0);
    }
    for _ in 0..150 {
        let f = fd("truncfile", CREATE | WRONLY | TRUNC);
        need(write(f, b"xxx") == 3, "write != 3");
        close(f);
    }
    let mut st = -1;
    wait(&mut st);
    unlink("truncfile");
    exit(st);
}
pub unsafe fn iputtest(_: &str) {
    need(mkdir("iputdir") == 0, "mkdir failed");
    need(chdir("iputdir") == 0, "chdir failed");
    need(unlink("../iputdir") == 0, "unlink cwd failed");
    need(chdir("/") == 0, "chdir / failed");
}
pub unsafe fn exitiputtest(s: &str) {
    if spawn() == 0 {
        iputtest_without_chdir(s);
        exit(0);
    }
    child_ok();
}
unsafe fn iputtest_without_chdir(_: &str) {
    need(mkdir("iputdir") == 0, "mkdir failed");
    need(chdir("iputdir") == 0, "chdir failed");
    need(unlink("../iputdir") == 0, "unlink cwd failed");
}
pub unsafe fn openiputtest(_: &str) {
    need(mkdir("oidir") == 0, "mkdir oidir failed");
    if spawn() == 0 {
        need(open("oidir", RDWR) < 0, "directory opened for write");
        exit(0);
    }
    pause(1);
    need(unlink("oidir") == 0, "unlink oidir failed");
    child_ok();
}
pub unsafe fn opentest(_: &str) {
    close(fd("/bin/echo", 0));
    need(open("doesnotexist", 0) < 0, "nonexistent file opened");
}
pub unsafe fn writetest(_: &str) {
    let f = fd("small", CREATE | RDWR);
    for _ in 0..100 {
        need(write(f, b"aaaaaaaaaa") == 10, "write aa failed");
        need(write(f, b"bbbbbbbbbb") == 10, "write bb failed");
    }
    close(f);
    let f = fd("small", 0);
    let b = &mut *(&raw mut BUF);
    need(read(f, &mut b[..2000]) == 2000, "read small failed");
    close(f);
    need(unlink("small") == 0, "unlink small failed");
}
pub unsafe fn writebig(_: &str) {
    let b = &mut *(&raw mut BUF);
    let f = fd("big", CREATE | RDWR);
    for i in 0..MAX_FILE {
        b[..4].copy_from_slice(&i.to_ne_bytes());
        need(
            write(f, &b[..BLOCK_SIZE as usize]) == BLOCK_SIZE as isize,
            "write big failed",
        );
    }
    close(f);
    let f = fd("big", 0);
    let mut n = 0;
    loop {
        let r = read(f, &mut b[..BLOCK_SIZE as usize]);
        if r == 0 {
            break;
        }
        need(r == BLOCK_SIZE as isize, "short read big");
        need(
            u32::from_ne_bytes(b[..4].try_into().unwrap()) == n,
            "wrong block data",
        );
        n += 1;
    }
    need(n == MAX_FILE, "wrong block count");
    close(f);
    need(unlink("big") == 0, "unlink big failed");
}
pub unsafe fn createtest(_: &str) {
    for i in 0..52 {
        close(open(name(&[b'a', b'0' + i]), CREATE | RDWR));
    }
    for i in 0..52 {
        unlink(name(&[b'a', b'0' + i]));
    }
}
pub unsafe fn dirtest(_: &str) {
    need(mkdir("dir0") == 0, "mkdir failed");
    need(chdir("dir0") == 0, "chdir failed");
    need(chdir("..") == 0, "chdir .. failed");
    need(unlink("dir0") == 0, "unlink dir0 failed");
}
pub unsafe fn exectest(_: &str) {
    unlink("echo-ok");
    let p = spawn();
    if p == 0 {
        let err = dup(1);
        need(err >= 0, "dup failed");
        close(1);
        need(fd("echo-ok", CREATE | WRONLY) == 1, "wrong stdout fd");
        exec("/bin/echo", &["echo", "OK"]);
        write(err, b"exec echo failed\n");
        exit(1);
    }
    let mut st = -1;
    need(wait(&mut st) == p, "wrong child pid");
    need(st == 0, "echo child failed");
    let f = fd("echo-ok", 0);
    let mut b = [0; 2];
    need(read(f, &mut b) == 2, "read echo failed");
    unlink("echo-ok");
    need(&b == b"OK", "wrong echo output");
    close(f);
}
pub unsafe fn sharedfd(_: &str) {
    unlink("sharedfd");
    let f = fd("sharedfd", CREATE | RDWR);
    let p = spawn();
    let mut b = [if p == 0 { b'c' } else { b'p' }; 10];
    for _ in 0..1000 {
        need(write(f, &b) == 10, "shared write failed");
    }
    if p == 0 {
        exit(0)
    }
    child_ok();
    close(f);
    let f = fd("sharedfd", 0);
    let (mut nc, mut np) = (0, 0);
    loop {
        let n = read(f, &mut b);
        if n <= 0 {
            break;
        }
        for c in &b[..n as usize] {
            if *c == b'c' {
                nc += 1;
            }
            if *c == b'p' {
                np += 1;
            }
        }
    }
    close(f);
    unlink("sharedfd");
    need(nc == 10000 && np == 10000, "shared offset or data mismatch");
}
pub unsafe fn fourfiles(_: &str) {
    let b = &mut *(&raw mut BUF);
    let names = ["f0", "f1", "f2", "f3"];
    for (i, n) in names.iter().enumerate() {
        unlink(n);
        if spawn() == 0 {
            let f = fd(n, CREATE | RDWR);
            b[..500].fill(b'0' + i as u8);
            for _ in 0..12 {
                need(write(f, &b[..500]) == 500, "fourfiles write failed");
            }
            exit(0);
        }
    }
    for _ in 0..4 {
        child_ok();
    }
    for (i, n) in names.iter().enumerate() {
        let f = fd(n, 0);
        let mut total = 0;
        loop {
            let r = read(f, b);
            if r <= 0 {
                break;
            }
            need(
                b[..r as usize].iter().all(|c| *c == b'0' + i as u8),
                "fourfiles data mismatch",
            );
            total += r;
        }
        close(f);
        need(total == 6000, "fourfiles size mismatch");
        unlink(n);
    }
}
pub unsafe fn createdelete(_: &str) {
    for pi in 0..4 {
        if spawn() == 0 {
            for i in 0..20 {
                close(fd(name(&[b'p' + pi, b'0' + i]), CREATE | RDWR));
                if i > 0 && i % 2 == 0 {
                    need(
                        unlink(name(&[b'p' + pi, b'0' + i / 2])) == 0,
                        "concurrent unlink failed",
                    );
                }
            }
            exit(0);
        }
    }
    for _ in 0..4 {
        child_ok();
    }
    for i in 0..20 {
        for pi in 0..4 {
            let f = open(name(&[b'p' + pi, b'0' + i]), 0);
            need(
                (f >= 0) == (i == 0 || i >= 10),
                "createdelete existence mismatch",
            );
            close(f);
        }
    }
    for i in 0..20 {
        for pi in 0..4 {
            unlink(name(&[b'p' + pi, b'0' + i]));
        }
    }
}
pub unsafe fn unlinkread(_: &str) {
    let f = fd("unlinkread", CREATE | RDWR);
    write(f, b"hello");
    close(f);
    let f = fd("unlinkread", RDWR);
    need(unlink("unlinkread") == 0, "unlink failed");
    let f1 = fd("unlinkread", CREATE | RDWR);
    write(f1, b"yyy");
    close(f1);
    let b = &mut *(&raw mut BUF);
    need(read(f, b) == 5, "unlinked file read failed");
    need(b[0] == b'h', "unlinked data mismatch");
    need(write(f, &b[..10]) == 10, "unlinked write failed");
    close(f);
    unlink("unlinkread");
}
pub unsafe fn linktest(_: &str) {
    unlink("lf1");
    unlink("lf2");
    let f = fd("lf1", CREATE | RDWR);
    need(write(f, b"hello") == 5, "write failed");
    close(f);
    need(link("lf1", "lf2") == 0, "link failed");
    unlink("lf1");
    need(open("lf1", 0) < 0, "unlinked name exists");
    let f = fd("lf2", 0);
    need(read(f, &mut *(&raw mut BUF)) == 5, "hard link data missing");
    close(f);
    need(link("lf2", "lf2") < 0, "self link succeeded");
    unlink("lf2");
    need(link("lf2", "lf1") < 0, "missing source linked");
    need(link(".", "lf1") < 0, "directory hard link succeeded");
}
pub unsafe fn concreate(_: &str) {
    for i in 0..40 {
        let bytes = [b'C', b'0' + i];
        let n = name(&bytes);
        unlink(n);
        let p = spawn();
        if (p != 0 && i % 3 == 1) || (p == 0 && i % 5 == 1) {
            link("C0", n);
        } else {
            close(fd(n, CREATE | RDWR));
        }
        if p == 0 {
            exit(0)
        }
        child_ok();
    }
    let mut seen = [false; 40];
    let mut de = DirEntry::new();
    let f = fd(".", 0);
    let mut n = 0;
    while read(f, api::utils::as_bytes_mut(&mut de)) > 0 {
        if de.inum != 0 && de.name[0] == b'C' && de.name[2] == 0 {
            let i = de.name[1].wrapping_sub(b'0') as usize;
            need(i < 40, "unexpected directory name");
            need(!seen[i], "duplicate directory entry");
            seen[i] = true;
            n += 1;
        }
    }
    close(f);
    need(n == 40, "missing directory entries");
    for i in 0..40 {
        let bytes = [b'C', b'0' + i];
        let n = name(&bytes);
        let p = spawn();
        if (i % 3 == 0 && p == 0) || (i % 3 == 1 && p != 0) {
            for _ in 0..6 {
                close(open(n, 0));
            }
        } else {
            for _ in 0..6 {
                unlink(n);
            }
        }
        if p == 0 {
            exit(0)
        }
        reap();
    }
}
pub unsafe fn linkunlink(_: &str) {
    unlink("x");
    let p = spawn();
    let mut x = if p == 0 { 97u32 } else { 1 };
    for _ in 0..100 {
        x = x.wrapping_mul(1103515245).wrapping_add(12345);
        match x % 3 {
            0 => close(open("x", RDWR | CREATE)),
            1 => {
                link("/bin/cat", "x");
            }
            _ => {
                unlink("x");
            }
        }
    }
    if p == 0 {
        exit(0)
    }
    reap();
}
pub unsafe fn subdir(_: &str) {
    let b = &mut *(&raw mut BUF);
    unlink("ff");
    need(mkdir("dd") == 0, "mkdir dd failed");
    let f = fd("dd/ff", CREATE | RDWR);
    write(f, b"ff");
    close(f);
    need(unlink("dd") < 0, "removed nonempty dd");
    need(mkdir("/dd/dd") == 0, "mkdir dd/dd failed");
    let f = fd("dd/dd/ff", CREATE | RDWR);
    write(f, b"FF");
    close(f);
    let f = fd("dd/dd/../ff", 0);
    need(read(f, b) == 2 && b[0] == b'f', ".. lookup data mismatch");
    close(f);
    need(link("dd/dd/ff", "dd/dd/ffff") == 0, "subdir link failed");
    need(unlink("dd/dd/ff") == 0, "subdir unlink failed");
    need(open("dd/dd/ff", 0) < 0, "unlinked file exists");
    for n in ["dd", "dd/../../dd", "dd/../../../dd", "./.."] {
        need(chdir(n) == 0, "relative chdir failed");
    }
    let f = fd("dd/dd/ffff", 0);
    need(read(f, b) == 2, "linked file size mismatch");
    close(f);
    for (n, m) in [
        ("dd/dd/ff", 0),
        ("dd/ff/ff", CREATE | RDWR),
        ("dd/xx/ff", CREATE | RDWR),
        ("dd", CREATE),
        ("dd", RDWR),
        ("dd", WRONLY),
    ] {
        need(open(n, m) < 0, "invalid subdir open succeeded");
    }
    for (a, b) in [
        ("dd/ff/ff", "dd/dd/xx"),
        ("dd/xx/ff", "dd/dd/xx"),
        ("dd/ff", "dd/dd/ffff"),
    ] {
        need(link(a, b) < 0, "invalid subdir link succeeded");
    }
    for n in ["dd/ff/ff", "dd/xx/ff", "dd/dd/ffff"] {
        need(mkdir(n) < 0, "invalid subdir mkdir succeeded");
    }
    for n in ["dd/xx/ff", "dd/ff/ff"] {
        need(unlink(n) < 0, "invalid unlink succeeded");
    }
    for n in ["dd/ff", "dd/xx"] {
        need(chdir(n) < 0, "invalid chdir succeeded");
    }
    need(unlink("dd/dd/ffff") == 0, "unlink ffff failed");
    need(unlink("dd/ff") == 0, "unlink ff failed");
    need(unlink("dd") < 0, "removed nonempty dd");
    need(unlink("dd/dd") == 0, "unlink child dir failed");
    need(unlink("dd") == 0, "unlink dd failed");
}
pub unsafe fn bigwrite(_: &str) {
    let b = &*(&raw const BUF);
    unlink("bigwrite");
    for size in (499..BUFSZ).step_by(471) {
        let f = fd("bigwrite", CREATE | RDWR);
        for _ in 0..2 {
            need(write(f, &b[..size]) == size as isize, "large write failed");
        }
        close(f);
        unlink("bigwrite");
    }
}
pub unsafe fn bigfile(_: &str) {
    let b = &mut *(&raw mut BUF);
    unlink("bigfile.dat");
    let f = fd("bigfile.dat", CREATE | RDWR);
    for i in 0..20 {
        b[..600].fill(i);
        need(write(f, &b[..600]) == 600, "bigfile write failed");
    }
    close(f);
    let f = fd("bigfile.dat", 0);
    let mut total = 0;
    let mut i = 0;
    loop {
        let n = read(f, &mut b[..300]);
        need(n >= 0, "bigfile read failed");
        if n == 0 {
            break;
        }
        need(n == 300, "bigfile short read");
        need(b[0] == i / 2 && b[299] == i / 2, "bigfile data mismatch");
        i += 1;
        total += n;
    }
    close(f);
    need(total == 12000, "bigfile wrong size");
    unlink("bigfile.dat");
}
pub unsafe fn fourteen(_: &str) {
    // 名前の上限はC版の14ではなく、共有APIのDIR_SZに合わせます。
    let raw = [b'1'; DIR_SZ + 1];
    let short = name(&raw[..DIR_SZ]);
    let long = name(&raw);
    need(mkdir(short) == 0, "mkdir maximum name failed");
    let inner = alloc::format!("{short}/{long}");
    need(mkdir(&inner) == 0, "mkdir long name failed");
    let path = alloc::format!("{long}/{long}/{long}");
    close(fd(&path, CREATE));
    let path2 = alloc::format!("{short}/{short}/{short}");
    close(fd(&path2, 0));
    need(
        mkdir(&alloc::format!("{short}/{short}")) < 0,
        "duplicate maximum name accepted",
    );
    need(
        mkdir(&alloc::format!("{long}/{short}")) < 0,
        "duplicate long name accepted",
    );
    unlink(&path2);
    unlink(&inner);
    unlink(short);
}
pub unsafe fn rmdot(_: &str) {
    need(mkdir("dots") == 0, "mkdir dots failed");
    need(chdir("dots") == 0, "chdir dots failed");
    need(unlink(".") < 0, "removed .");
    need(unlink("..") < 0, "removed ..");
    need(chdir("/") == 0, "chdir / failed");
    need(unlink("dots/.") < 0, "removed dots/.");
    need(unlink("dots/..") < 0, "removed dots/..");
    need(unlink("dots") == 0, "unlink dots failed");
}
pub unsafe fn dirfile(_: &str) {
    close(fd("dirfile", CREATE));
    need(chdir("dirfile") < 0, "chdir regular file succeeded");
    need(open("dirfile/xx", 0) < 0, "opened child of file");
    need(open("dirfile/xx", CREATE) < 0, "created child of file");
    need(mkdir("dirfile/xx") < 0, "mkdir under file succeeded");
    need(unlink("dirfile/xx") < 0, "unlink under file succeeded");
    need(
        link("/bin/echo", "dirfile/xx") < 0,
        "link under file succeeded",
    );
    need(unlink("dirfile") == 0, "unlink failed");
    need(open(".", RDWR) < 0, "opened directory for write");
    let f = fd(".", 0);
    need(write(f, b"x") <= 0, "wrote directory");
    close(f);
}
pub unsafe fn iref(_: &str) {
    for _ in 0..NINODE + 1 {
        need(mkdir("irefd") == 0, "mkdir irefd failed");
        need(chdir("irefd") == 0, "chdir irefd failed");
        mkdir("");
        link("/bin/echo", "");
        close(open("", CREATE));
        close(open("xx", CREATE));
        unlink("xx");
    }
    for _ in 0..NINODE + 1 {
        chdir("..");
        unlink("irefd");
    }
    chdir("/");
}
pub unsafe fn partial_write(_: &str) {
    unlink("testfile");
    let f = fd("testfile", CREATE | RDWR);
    need(write(f, b"A") == 1, "write A failed");
    close(f);
    let f = fd("testfile", RDWR);
    brk((PG_SIZE - brk(0) % PG_SIZE) as isize);
    let p = brk(0);
    need(p % PG_SIZE == 0, "unaligned break");
    store(p - 1, b'X');
    need(
        sys_write(f as u32, (p - 1) as *const u8, 2) == -1,
        "partial write should fail",
    );
    close(f);
    let mut b = [0; 1];
    let f = fd("testfile", 0);
    need(
        read(f, &mut b) == 1 && b[0] == b'X',
        "partial update not visible",
    );
    close(f);
    let f = fd("bigfile", CREATE | RDWR);
    let b = &mut *(&raw mut BUF);
    b[..1024].fill(0);
    for _ in 0..64 {
        need(write(f, &b[..1024]) == 1024, "cache flushing write failed");
    }
    close(f);
    unlink("bigfile");
    let f = fd("testfile", 0);
    need(
        read(f, &mut b[..1]) == 1 && b[0] == b'X',
        "partial update not persistent",
    );
    close(f);
    unlink("testfile");
}
pub unsafe fn unlinkcwd(_: &str) {
    need(mkdir("/a") == 0, "mkdir /a failed");
    need(mkdir("/a/b") == 0, "mkdir /a/b failed");
    need(chdir("/a/b") == 0, "chdir failed");
    need(unlink("/a/b") == 0, "unlink /a/b failed");
    need(unlink("/a") == 0, "unlink /a failed");
    if open("../", 0) > 0 {
        println!("open ../ non-existing directory");
    }
    if open("../c", CREATE) > 0 {
        println!("create ../c non-existing file");
    }
}
pub unsafe fn bigdir(_: &str) {
    unlink("bd");
    close(fd("bd", CREATE));
    for i in 0..500 {
        need(
            link(
                "bd",
                name(&[b'x', b'0' + (i / 64) as u8, b'0' + (i % 64) as u8]),
            ) == 0,
            "bigdir link failed",
        );
    }
    unlink("bd");
    for i in 0..500 {
        need(
            unlink(name(&[b'x', b'0' + (i / 64) as u8, b'0' + (i % 64) as u8])) == 0,
            "bigdir unlink failed",
        );
    }
}
pub unsafe fn manywrites(_: &str) {
    for ci in 0..4 {
        if spawn() == 0 {
            let bytes = [b'b', b'a' + ci];
            let n = name(&bytes);
            unlink(n);
            for _ in 0..30 {
                for _ in 0..ci + 1 {
                    let f = fd(n, CREATE | RDWR);
                    need(
                        write(f, &*(&raw const BUF)) == BUFSZ as isize,
                        "manywrites write failed",
                    );
                    close(f);
                }
                unlink(n);
            }
            unlink(n);
            exit(0);
        }
    }
    for _ in 0..4 {
        child_ok();
    }
}
pub unsafe fn badwrite(_: &str) {
    unlink("junk");
    for _ in 0..600 {
        let f = fd("junk", CREATE | WRONLY);
        sys_write(f as u32, 0xffffffffffusize as *const u8, 1);
        close(f);
        unlink("junk");
    }
    let f = fd("junk", CREATE | WRONLY);
    need(write(f, b"x") == 1, "write after invalid writes failed");
    close(f);
    unlink("junk");
}
pub unsafe fn diskfull(_: &str) {
    unlink("diskfulldir");
    let b = &*(&raw const BUF);
    let mut done = false;
    for i in 0..79 {
        if done {
            break;
        }
        let bytes = [b'b', b'i', b'g', b'0' + i];
        let n = name(&bytes);
        unlink(n);
        let f = open(n, CREATE | RDWR | TRUNC);
        if f < 0 {
            println!("could not create file {}", n);
            break;
        }
        for _ in 0..MAX_FILE {
            if write(f, &b[..BLOCK_SIZE as usize]) != BLOCK_SIZE as isize {
                done = true;
                break;
            }
        }
        close(f);
    }
    for i in 0..128 {
        let bytes = [b'z', b'z', b'0' + (i / 32) as u8, b'0' + (i % 32) as u8];
        let n = name(&bytes);
        unlink(n);
        let f = open(n, CREATE | RDWR | TRUNC);
        if f < 0 {
            break;
        }
        close(f);
    }
    if mkdir("diskfulldir") == 0 {
        println!("mkdir(diskfulldir) unexpectedly succeeded!");
    }
    unlink("diskfulldir");
    for i in 0..128 {
        unlink(name(&[
            b'z',
            b'z',
            b'0' + (i / 32) as u8,
            b'0' + (i % 32) as u8,
        ]));
    }
    for i in 0..79 {
        unlink(name(&[b'b', b'i', b'g', b'0' + i]));
    }
}
pub unsafe fn outofinodes(_: &str) {
    for i in 0..1024 {
        let bytes = [b'z', b'z', b'0' + (i / 32) as u8, b'0' + (i % 32) as u8];
        let n = name(&bytes);
        unlink(n);
        let f = open(n, CREATE | RDWR | TRUNC);
        if f < 0 {
            break;
        }
        close(f);
    }
    for i in 0..1024 {
        unlink(name(&[
            b'z',
            b'z',
            b'0' + (i / 32) as u8,
            b'0' + (i % 32) as u8,
        ]));
    }
}
