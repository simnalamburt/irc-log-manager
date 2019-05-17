use memmap::MmapOptions;
use std::env;
use std::error;
use std::fs::File;
use std::io::prelude::*;
use std::io::{BufReader, Cursor};
use std::time::SystemTime;

fn main() -> Result<(), Box<error::Error>> {
    let file = File::open(format!(
        r"{}/.weechat/logs/irc.ozinger.#langdev.weechatlog",
        env::var("HOME")?
    ))?;

    //                    | dev   | release
    // -------------------|-------|---------
    // mmap() with for    | ~8.0s | ~65.4ms
    // mmap() with Cursor | ~2.3s | ~275ms
    // BufReader          | ~2.1s | ~300ms

    let now = SystemTime::now();
    let mmap = unsafe { MmapOptions::new().map(&file)? };
    let mut count = 0;
    for i in 0..mmap.len() {
        if mmap[i] == b'\n' {
            count += 1;
        }
    }
    println!("{}, {:?}", count, now.elapsed()?);

    let now = SystemTime::now();
    let mut count = 0;
    for _ in Cursor::new(mmap).lines() {
        count += 1;
    }
    println!("{}, {:?}", count, now.elapsed()?);

    let now = SystemTime::now();
    let mut count = 0;
    for _ in BufReader::new(file).lines() {
        count += 1;
    }
    println!("{}, {:?}", count, now.elapsed()?);

    Ok(())
}
