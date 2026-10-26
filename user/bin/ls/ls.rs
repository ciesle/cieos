#![no_std]
#![no_main]

extern crate alloc;
use alloc::boxed::Box;
use alloc::format;
use api::file::*;
use api::param::DIR_SZ;
use api::utils::*;
use core::error::Error;

use ulib::file::*;
use ulib::print::*;

fn ls(path: &str) -> Result<(), Box<dyn Error>> {
    let Ok(mut fd) = File::open(path, FileOpenMode::RD_ONLY) else {
        return Err(format!("ls: cannot open {}", path).into());
    };

    let Ok(state) = fd.state() else {
        return Err(format!("ls: cannot stat {}", path).into());
    };

    let mut buf = [0u8; 512];

    match state.inode_type {
        InodeType::Device | InodeType::File => {
            println!(
                "name: {} | type: {} | inum: {} | size: {}",
                &path[path.rfind('/').map(|x| x + 1).unwrap_or(0)..],
                state.inode_type.as_str(),
                state.inum,
                state.size,
            );
        }
        InodeType::Directory => {
            if path.len() + 1 + DIR_SZ + 1 > buf.len() {
                println!("ls: path too long");
            } else {
                buf[..path.len()].copy_from_slice(path.as_bytes());
                let mut idx = path.len();
                buf[idx] = b'/';
                idx += 1;
                let mut de = DirEntry::new();
                while let Ok(n) = fd.read(as_bytes_mut(&mut de))
                    && n == size_of::<DirEntry>()
                {
                    if de.inum == 0 {
                        continue;
                    }
                    let name = &de.name[..de
                        .name
                        .iter()
                        .position(|&x| x == 0)
                        .unwrap_or(de.name.len())];
                    buf[idx..idx + name.len()].copy_from_slice(name);
                    let Ok(name) = core::str::from_utf8(&buf[..idx + name.len()]) else {
                        println!("ls: cannot format name");
                        continue;
                    };
                    let Ok(state) =
                        File::open(name, FileOpenMode::RD_ONLY).and_then(|fd| fd.state())
                    else {
                        println!("ls: cannot stat {}", name);
                        continue;
                    };
                    println!(
                        "name: {} | type: {} | inum: {} | size: {}",
                        &name[name.rfind('/').map(|x| x + 1).unwrap_or(0)..],
                        state.inode_type.as_str(),
                        state.inum,
                        state.size,
                    );
                }
            }
        }
        _ => {}
    }
    Ok(())
}

#[unsafe(no_mangle)]
fn main(argv: &[&str]) -> Result<(), Box<dyn Error>> {
    if argv.len() < 2 {
        return ls(".");
    }
    for arg in &argv[1..] {
        match ls(arg) {
            Err(e) => fprintln!(2, "{e}"),
            _ => {}
        }
    }
    Ok(())
}
