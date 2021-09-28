use chromium_cache_parser::entry::Key;
use getopts::Options;

use std::path::Path;
use std::collections::HashSet;
use std::process::Command;
use std::{thread, time};
use std::env;

fn main() -> anyhow::Result<()> {
    let arg: Vec<String> = env::args().skip(1).collect();

    let mut opt = Options::new();
    opt.optflag("h", "help", "print this help");
    opt.optflag("s", "skip-stop", "disable wait while loop");
    opt.optflag("c", "check-files", "check if all file downloaded.");
    opt.optopt("o", "output", "output dir to save video", "OUTPUT");
    opt.optopt("q", "query", "query string to search video from cache", "QUERY");

    if arg.len() == 0 {
        print!("{}", opt.usage("Usage: cache2mp4 [[-q|--query] QUERY] [[-o|--output] OUTPUT]"));
        return Ok(());
    }

    let argparse = opt.parse(arg)?;

    if argparse.opt_present("h") {
        print!("{}", opt.usage("Usage: cache2mp4 [[-q|--query] QUERY] [[-o|--output] OUTPUT]"));
        return Ok(());
    }

    let check_mode = argparse.opt_present("c");

    if check_mode {
        let dst = argparse.opt_str("o").expect("Error: output option require argument");
        let dst = Path::new(&dst);
        let m3u8 = glob::glob(dst.join("*.m3u8").to_str().unwrap()).unwrap().filter_map(Result::ok).next();

        if let Some(m3u8_path) = m3u8 {
            let raw_str = std::fs::read_to_string(m3u8_path)?;
            let target_all: HashSet<String> = raw_str.lines().filter(|x| !x.starts_with('#')).map(|x| x.into()).collect();
            let mut target_all: Vec<String> = target_all.into_iter().collect();
            target_all.sort();

            println!("missing file found from m3u8 file:");

            for target in target_all {
                if !dst.join(&target).exists() {
                    println!("{}", target);
                }
            }
        } else {
            println!("Error: m3u8 file not found in {}", dst.display());
        }

        return Ok(());
    }

    let wait_flag = !argparse.opt_present("s");

    let search_str = argparse.opt_str("q").expect("Error: query option require argument");
    let dst = argparse.opt_str("o").expect("Error: output option require argument");
    let dst = Path::new(&dst);

    std::fs::create_dir_all(dst)?;

    let mut m3u8 = None;
    let mut target: Option<HashSet<String>> = None;
    let mut saved = HashSet::new();

    let wait_sec = time::Duration::from_secs(180);

    for file in glob::glob(dst.join("*").to_str().unwrap()).unwrap().filter_map(Result::ok) {
        if let Some(s) = file.file_name() {
            let s_s = s.to_str().unwrap().to_string();
            if s_s.ends_with(".m3u8") { m3u8 = Some(s_s.clone()) }
            saved.insert(s_s);
        }
    }

    if let Some(fname) = &m3u8 {
        let raw_str = std::fs::read_to_string(dst.join(fname))?;
        let mut target_all: HashSet<String> = raw_str.lines().filter(|x| !x.starts_with('#')).map(|x| x.into()).collect();

        for name in &saved {
            if target_all.contains(name) {
                target_all.remove(name);
            }
        }
        target = Some(target_all);
    }

    while target.is_none() || !target.as_ref().unwrap().is_empty() {
        if wait_flag {
            thread::sleep(wait_sec);
        }

        let result = chromium_cache_parser::parse("/mnt/c/Users/kimot/AppData/Local/Vivaldi/User Data/Default/Cache/index");
        if let Err(e) = result {
            println!("Warning: an error occured while parsing index file.");
            println!("{}", e);
            continue;
        }

        let result = result.unwrap();

        for cache in &result.entries {
            if let Key::LocalKey(key) = &cache.key {
                let fname = key.split('/').last();

                if let Some(fname_str) = fname {
                    if let Some(dict) = &mut target {
                        if !dict.contains(key) && key.contains(&search_str) && !cache.data.is_empty() {
                            if let Err(e) = result.copy_data(&cache, dst) {
                                println!("{}", e);
                                continue;
                            }
                            dict.remove(fname_str);
                        }
                    } else {
                        if key.contains(&search_str) && !cache.data.is_empty() {
                            if let Err(e) = result.copy_data(&cache, dst) {
                                println!("{}", e);
                                continue;
                            }

                            if key.ends_with(".m3u8") {
                                m3u8 = Some(fname_str.into());
                            }
                            saved.insert(fname_str.into());
                        }
                    }
                }
            }
        }

        if target.is_none() {
            if let Some(fname) = &m3u8 {
                let raw_str = std::fs::read_to_string(dst.join(fname))?;
                let mut target_all: HashSet<String> = raw_str.lines().filter(|x| !x.starts_with('#')).map(|x| x.into()).collect();

                for name in &saved {
                    if target_all.contains(name) {
                        target_all.remove(name);
                    }
                }

                target = Some(target_all);
            }
        }
    }

    let mut child = Command::new("ffmpeg").args(&[
        "-i", &dst.join(m3u8.unwrap()).into_os_string().into_string().unwrap(),
        "-movflags", "faststart",
        "-c", "copy",
        "-bsf:a", "aac_adtstoasc",
        &dst.parent().unwrap().join(dst.file_name().unwrap()).with_extension("mp4").into_os_string().into_string().unwrap()
    ]).spawn().expect("unable to spawn child process");

    println!("ffmpeg finished with code {}", child.wait().unwrap());

    Ok(())
}

