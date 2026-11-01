use crate::capture::Shot;
use crate::config::{Config, Rect, Res};
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub fn encode_png(shot: &Shot, r: Rect) -> Res<Vec<u8>> {
    let mut rgb = Vec::with_capacity(r.w as usize * r.h as usize * 3);
    for y in r.y as u32..r.y as u32 + r.h {
        let row = y as usize * shot.w as usize * 4;
        for x in r.x as usize..r.x as usize + r.w as usize {
            let i = row + x * 4;
            rgb.extend_from_slice(&[shot.data[i + 2], shot.data[i + 1], shot.data[i]]);
        }
    }
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, r.w, r.h);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::Fast);
    enc.write_header()?.write_image_data(&rgb)?;
    Ok(out)
}

pub fn copy_to_clipboard(mime: &str, bytes: &[u8]) -> Res<()> {
    let mut child = Command::new("xclip")
        .args(["-selection", "clipboard", "-t", mime])
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("xclip is needed for the clipboard: {e}"))?;
    child.stdin.take().ok_or("xclip has no stdin")?.write_all(bytes)?;
    if !child.wait()?.success() {
        return Err("xclip failed".into());
    }
    Ok(())
}

pub fn timestamped(dir: &Path, ext: &str) -> Res<PathBuf> {
    fs::create_dir_all(dir)?;
    let out = Command::new("date").arg("+%Y-%m-%d_%H-%M-%S").output()?;
    let stamp = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if stamp.is_empty() {
        return Err("could not read the current time".into());
    }
    Ok(dir.join(format!("{stamp}.{ext}")))
}

pub fn runtime_dir() -> PathBuf {
    env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

pub fn notify(cfg: &Config, title: &str, body: &str) {
    if cfg.notify {
        let _ = Command::new("notify-send")
            .args([title, body])
            .stderr(Stdio::null())
            .status();
    }
}
