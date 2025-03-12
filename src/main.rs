use clap::{App, AppSettings, clap_app, crate_version};
use failure::{Fail, Fallible, bail};
use memmap::{Mmap, MmapOptions};
use rayon::prelude::*;
use regex::Regex;
use std::env;
use std::fs::File;
use std::io::BufReader;
use std::io::prelude::*;
use std::str::from_utf8;

fn main() -> Fallible<()> {
    let matches = clap_app!(
        @app(App::new("irc-log-manager"))
        (version: crate_version!())
        (about: "Manage IRC logs of weechat")
        (@subcommand check =>
            (about: "check if IRC logs are intact")
            (@arg verbose: -v --verbose "Check IRC logs verbosely")
        )
        (@subcommand sort =>
            (about: "sort IRC channels in order of recent activity")
            (@arg verbose: -v --verbose "Process IRC logs verbosely")
        )
    )
    .setting(AppSettings::ArgRequiredElseHelp)
    .get_matches();

    match matches.subcommand() {
        ("check", Some(m)) => check(m.is_present("verbose")),
        ("sort", Some(m)) => sort(m.is_present("verbose")),
        _ => unreachable!(),
    }
}

fn logs_into_par_iter(
    verbose: bool,
) -> Fallible<impl IndexedParallelIterator<Item = Fallible<(String, String, u32, Mmap)>>> {
    let home_dir = env::var("HOME")?;
    let path_cfg = format!("{}/.weechat/weechat.conf", home_dir);
    let file_cfg = File::open(path_cfg)?;
    let re = Regex::new(r#"^default\.buffer = "irc;(\w+?)\.#([-\w]+?);(\d+)"$"#)?;

    let channels: Vec<_> = BufReader::new(file_cfg)
        .lines()
        .filter_map(|line| match line {
            Ok(line) => {
                let caps = re.captures(&line)?;
                let server = &caps[1];
                let channel = &caps[2];
                let index: u32 = caps[3].parse().ok()?;
                if verbose {
                    eprintln!(r#"Found channel "{}""#, channel);
                }
                Some(Ok((server.to_string(), channel.to_string(), index)))
            }
            Err(err) => Some(Err(err)),
        })
        .collect::<Result<_, _>>()?;

    if verbose {
        eprintln!("Found {} channels", channels.len());
    }

    let iter = channels
        .into_par_iter()
        .map(move |(server, channel, index)| -> Fallible<_> {
            let path = format!(
                "{}/.weechat/logs/irc.{}.#{}.weechatlog",
                home_dir,
                server.to_lowercase(),
                channel.to_lowercase()
            );
            let file = File::open(&path);
            if file.is_err() && verbose {
                eprintln!(r#"Failed to read "{}"#, path);
            }
            let mmap = unsafe { MmapOptions::new().map(&file?)? };
            Ok((server, channel, index, mmap))
        });

    Ok(iter)
}

#[derive(Debug, Fail)]
enum Error {
    #[fail(
        display = r#"File "{}" has an unexpected format in {}th byte. ("{}")"#,
        file_name, position, line
    )]
    UnexpectedFormat {
        file_name: String,
        position: usize,
        line: String,
    },
}

fn check(verbose: bool) -> Fallible<()> {
    let re = Regex::new(r"^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\t$")?;

    let iter = logs_into_par_iter(verbose)?;
    let count = iter.len();
    iter.map(|param| -> Fallible<()> {
        let (server, channel, _, mmap) = param?;
        if verbose {
            eprintln!(r#"Checking #{} of {}"#, channel, server);
        }

        for i in 0..mmap.len() {
            if mmap[i] != b'\n' {
                continue;
            }

            let end = i + 21;
            if end > mmap.len() {
                continue;
            }

            let line = from_utf8(&mmap[i..end])?;
            if !re.is_match(line) {
                continue;
            }

            let file_name = format!(
                "irc.{}.#{}.weechatlog",
                server.to_lowercase(),
                channel.to_lowercase()
            );
            if verbose {
                eprintln!(
                    r#"Found unexpected format in the {}th byte of file "{}""#,
                    i, file_name
                );
            }
            bail!(Error::UnexpectedFormat {
                file_name,
                position: i,
                line: line.to_string(),
            })
        }
        if verbose {
            let file_name = format!(
                "irc.{}.#{}.weechatlog",
                server.to_lowercase(),
                channel.to_lowercase()
            );
            eprintln!(r#"Finished checking "{}""#, file_name);
        }
        Ok(())
    })
    .collect::<Result<(), _>>()?;

    println!("Checked {} files, no issue was found", count);

    Ok(())
}

fn sort(verbose: bool) -> Fallible<()> {
    let re =
        Regex::new(r"^(?P<date>\d{4}-\d{2}-\d{2}) \d{2}:\d{2}:\d{2}\t@?(?P<name>.*?)(?:$|\t)")?;

    let mut result: Vec<_> = logs_into_par_iter(verbose)?
        .map(|param| {
            let (server, channel, index, mmap) = param?;
            if verbose {
                let file_name = format!(
                    "irc.{}.#{}.weechatlog",
                    server.to_lowercase(),
                    channel.to_lowercase()
                );
                eprintln!(r#"Checking "{}""#, file_name);
            }
            let mut last_newline = None;
            let mut count = 0;

            for i in (0..mmap.len()).rev() {
                if mmap[i] != b'\n' {
                    continue;
                }

                if let Some(end) = last_newline {
                    last_newline = Some(i);
                    let line = from_utf8(&mmap[i + 1..end])?;

                    let caps = re.captures(line).ok_or_else(|| Error::UnexpectedFormat {
                        file_name: format!(
                            "irc.{}.#{}.weechatlog",
                            server.to_lowercase(),
                            channel.to_lowercase()
                        ),
                        position: i,
                        line: line.to_string(),
                    })?;

                    if &caps["date"] < "2019-02-18" {
                        break;
                    }

                    let name = &caps["name"];
                    if let "김젼" | "김지현" | "지현" | "지현_" = name {
                        count += 1
                    }
                } else {
                    last_newline = Some(i);
                    continue;
                }
            }
            if verbose {
                let file_name = format!(
                    "irc.{}.#{}.weechatlog",
                    server.to_lowercase(),
                    channel.to_lowercase()
                );
                eprintln!(
                    r#"Finished processing "{}" at {}th byte"#,
                    file_name,
                    last_newline.unwrap(),
                );
            }
            Ok((server, channel, count, index))
        })
        .collect::<Fallible<_>>()?;

    if verbose {
        eprintln!("Processed {} files", result.len());
    }

    result.sort_unstable_by_key(|e| (-e.2, e.3));
    for (i, (server, channel, ..)) in result.into_iter().enumerate() {
        let new_index = i + 2;

        println!("/buffer {}.#{}", server, channel);
        println!("/buffer move {}", new_index);
    }

    Ok(())
}
