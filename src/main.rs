mod capture;
mod config;
mod output;
mod record;
mod select;

use capture::{choose_area, clamp, grab};
use config::{Config, Mode, Rect, Res, USAGE};
use output::{copy_to_clipboard, encode_png, notify, timestamped};
use select::select;
use std::env;
use std::fs;
use std::thread::sleep;
use std::time::Duration;
use x11rb::rust_connection::RustConnection;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{USAGE}");
        return;
    }
    let cfg = match config::load(&args) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("i3snip: {e}\n");
            eprint!("{USAGE}");
            std::process::exit(2);
        }
    };
    if let Err(e) = run(&cfg) {
        eprintln!("i3snip: {e}");
        notify(&cfg, "i3snip failed", &e.to_string());
        std::process::exit(1);
    }
}

fn pick(cfg: &Config) -> Res<Option<(capture::Shot, Rect)>> {
    if cfg.delay > 0 {
        sleep(Duration::from_secs(cfg.delay));
    }
    let (conn, screen_num) = RustConnection::connect(None)?;
    let area = choose_area(&conn, screen_num, &cfg.monitor)?;
    let shot = grab(&conn, screen_num, &area)?;
    let rect = if cfg.full {
        Some(Rect {
            x: 0,
            y: 0,
            w: shot.w,
            h: shot.h,
        })
    } else if let Some(a) = cfg.area {
        Some(clamp(a, shot.w, shot.h).ok_or("--area is outside the screen")?)
    } else {
        select(&conn, screen_num, &shot, cfg)?
    };
    Ok(rect.map(|r| (shot, r)))
}

fn run(cfg: &Config) -> Res<()> {
    match cfg.mode {
        Mode::Stop => {
            if !record::stop(cfg)? {
                eprintln!("i3snip: no recording in progress");
            }
            Ok(())
        }
        Mode::Video => {
            if record::stop(cfg)? {
                return Ok(());
            }
            match pick(cfg)? {
                Some((shot, r)) => record::start(cfg, r, (shot.ox, shot.oy)),
                None => Ok(()),
            }
        }
        Mode::Photo => {
            let Some((shot, r)) = pick(cfg)? else { return Ok(()) };
            let png = encode_png(&shot, r)?;
            let mut body = String::from("copied to the clipboard");
            if cfg.save {
                let path = timestamped(&cfg.pictures_dir, "png")?;
                fs::write(&path, &png)?;
                body = path.display().to_string();
            }
            if cfg.clipboard {
                copy_to_clipboard("image/png", &png)?;
            }
            notify(cfg, "Screenshot taken", &body);
            Ok(())
        }
    }
}
