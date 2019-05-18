use failure::Fallible;
use memmap::MmapOptions;
use rayon::prelude::*;
use regex::Regex;
use std::env;
use std::fs::{read_dir, File};

fn main() -> Fallible<()> {
    let re = Regex::new(r"^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\t$")?;

    let home_dir = env::var("HOME")?;
    let log_dir = format!(r"{}/.weechat/logs/", home_dir);
    let entries: Vec<_> = read_dir(log_dir)?.collect();

    entries
        .into_par_iter()
        .map(|entry| -> Fallible<_> {
            let entry = entry?;
            let file = File::open(entry.path())?;
            let mmap = unsafe { MmapOptions::new().map(&file)? };

            // Create index
            for i in 0..mmap.len() {
                if mmap[i] != b'\n' {
                    continue;
                }

                let end = i + 21;
                if end > mmap.len() {
                    continue;
                }

                let words = std::str::from_utf8(&mmap[i..end])?;
                if !re.is_match(words) {
                    continue;
                }

                println!("File {:?} has issue with {}th byte", entry.file_name(), i);
                break;
            }

            Ok(())
        })
        .collect::<Result<_, _>>()?;

    Ok(())
}
