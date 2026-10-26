#![no_std]
#![no_main]

extern crate alloc;
use alloc::boxed::Box;
use core::error::Error;
use core::ptr::null_mut;

extern crate ulib;
use api::file::*;
use ulib::file::*;
use ulib::print::*;
use ulib::usys::*;

#[unsafe(no_mangle)]
fn main(_: &[&str]) -> Result<(), Box<dyn Error>> {
    let stdin = match File::open("/dev/console", FileOpenMode::RDWR) {
        Ok(cons) => cons,
        Err(_) => {
            mknod("/dev/console", 1, 0)?;
            File::open("/dev/console", FileOpenMode::RDWR)?
        }
    };
    let _stdout = stdin.try_clone()?;
    let _stderr = stdin.try_clone()?;
    let argv: [&str; 1] = ["/bin/sh"; 1];

    loop {
        println!("init: starting sh");
        let pid = match fork() {
            Err(()) => {
                println!("init: fork failed");
                exit(1)
            }
            Ok(0) => {
                let _ = exec("/bin/sh", &argv);
                println!("init: exec sh failed");
                exit(1)
            }
            Ok(pid) => pid,
        };

        loop {
            match wait(null_mut()) {
                Ok(wpid) if wpid == pid => {
                    break;
                    // シェルが死んだので、再開する
                }
                Err(_) => {
                    println!("init: wait returned an error");
                    exit(1);
                }
                _ => {}
            }
        }
    }
}
