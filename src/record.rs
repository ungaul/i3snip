use crate::config::{Config, Rect, Res};
use crate::output::{copy_to_clipboard, notify, runtime_dir, timestamped};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

fn pidfile() -> PathBuf {
    runtime_dir().join("i3snip-video.pid")
}

pub fn stop(cfg: &Config) -> Res<bool> {
    let Ok(text) = fs::read_to_string(pidfile()) else {
        return Ok(false);
    };
    let _ = fs::remove_file(pidfile());
    let mut lines = text.lines();
    let Some(pid) = lines.next().and_then(|l| l.trim().parse::<u32>().ok()) else {
        return Ok(false);
    };
    let file = lines.next().unwrap_or("").to_string();
    let proc = PathBuf::from(format!("/proc/{pid}"));
    if !proc.exists() {
        return Ok(false);
    }
    Command::new("kill").args(["-INT", &pid.to_string()]).status()?;
    for _ in 0..100 {
        if !proc.exists() {
            break;
        }
        sleep(Duration::from_millis(50));
    }
    if cfg.clipboard && !file.is_empty() {
        copy_to_clipboard("text/uri-list", format!("file://{file}\r\n").as_bytes())?;
    }
    notify(cfg, "Recording saved", &file);
    Ok(true)
}

pub fn start(cfg: &Config, r: Rect, origin: (i32, i32)) -> Res<()> {
    sleep(Duration::from_millis(150));
    let (w, h) = (r.w & !1, r.h & !1);
    if w == 0 || h == 0 {
        return Err("selection too small to record".into());
    }
    let display = env::var("DISPLAY").map_err(|_| "DISPLAY is not set")?;
    let path = timestamped(&cfg.videos_dir, &cfg.ext)?;

    let mut cmd = Command::new("ffmpeg");
    let (x, y) = (origin.0 + r.x, origin.1 + r.y);
    if cfg.encoder == "vaapi" {
        cmd.args(["-vaapi_device", "/dev/dri/renderD128"]);
    }
    cmd.args([
        "-loglevel",
        "error",
        "-f",
        "x11grab",
        "-framerate",
        &cfg.fps.to_string(),
    ]);
    cmd.args([
        "-thread_queue_size",
        "512",
        "-draw_mouse",
        if cfg.cursor { "1" } else { "0" },
    ]);
    cmd.args(["-video_size", &format!("{w}x{h}"), "-i", &format!("{display}+{x},{y}")]);
    if let Some(source) = &cfg.audio {
        cmd.args(["-f", "pulse", "-i", source]);
    }
    let crf = cfg.crf.to_string();
    match cfg.encoder.as_str() {
        "vaapi" => cmd.args(["-vf", "format=nv12,hwupload", "-c:v", "h264_vaapi", "-qp", &crf]),
        "vp9" => cmd.args([
            "-c:v",
            "libvpx-vp9",
            "-deadline",
            "realtime",
            "-cpu-used",
            "8",
            "-crf",
            &crf,
            "-b:v",
            "0",
        ]),
        _ => {
            cmd.args([
                "-c:v",
                "libx264",
                "-preset",
                &cfg.preset,
                "-crf",
                &crf,
                "-pix_fmt",
                "yuv420p",
            ]);
            match cfg.tune {
                Some(t) => cmd.args(["-tune", t]),
                None => &mut cmd,
            }
        }
    };
    if cfg.ext == "mp4" {
        cmd.args(["-movflags", "+frag_keyframe+empty_moov+default_base_moof"]);
    }
    let log_path = runtime_dir().join("i3snip-ffmpeg.log");
    let log = fs::File::create(&log_path)?;
    cmd.arg(&path).stdin(Stdio::null()).stdout(Stdio::null()).stderr(log);

    let mut child = cmd.spawn().map_err(|e| format!("ffmpeg is needed for video: {e}"))?;
    for _ in 0..30 {
        sleep(Duration::from_millis(50));
        if let Some(status) = child.try_wait()? {
            let _ = fs::remove_file(&path);
            return Err(format!("ffmpeg exited with {status}, see {}", log_path.display()).into());
        }
    }
    fs::write(pidfile(), format!("{}\n{}", child.id(), path.display()))?;
    notify(cfg, "Recording started", &path.display().to_string());
    Ok(())
}
