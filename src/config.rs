use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub type Res<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Photo,
    Video,
    Stop,
}

pub struct Config {
    pub mode: Mode,
    pub area: Option<Rect>,
    pub full: bool,
    pub delay: u64,
    pub dim: u32,
    pub border_color: u32,
    pub border_width: u16,
    pub pictures_dir: PathBuf,
    pub videos_dir: PathBuf,
    pub clipboard: bool,
    pub save: bool,
    pub notify: bool,
    pub fps: u32,
    pub crf: u32,
    pub preset: String,
    pub tune: Option<&'static str>,
    pub audio: Option<String>,
    pub ext: String,
    pub encoder: String,
    pub monitor: String,
    pub cursor: bool,
}

pub const USAGE: &str = "usage: i3snip <photo|video|stop> [options]

selection
  --area X,Y,W,H       skip the selector and use this region
  --full               capture the whole screen of the selected monitor
  --monitor WHICH      focus (monitor under the pointer, default), all, or an output name like eDP-1
  --delay SECONDS      wait before grabbing the screen

appearance
  --dim PERCENT        how much to darken outside the selection (default 45)
  --border-color HEX   selection border color (default 9aac6f)
  --border-width PX    selection border width (default 2)

output
  --pictures-dir DIR   default ~/Pictures/Screenshots
  --videos-dir DIR     default ~/Videos
  --no-clipboard       do not copy the photo or the video file to the clipboard
  --no-save            do not write photos to disk
  --no-notify          no notifications

video
  --profile NAME       light, balanced (default) or quality
                         light     hardware encoder when available, small files, low load (60 fps)
                         balanced  x264 veryfast, crf 23, 60 fps
                         quality   x264 medium, crf 18, 60 fps
  --fps N              override the profile frame rate
  --crf N              override the quality, lower is better (0-51)
  --preset NAME        override the x264 preset
  --audio [SOURCE]     record audio (default source when omitted)
  --ext EXT            container: mp4, mkv, webm (default mp4)
  --encoder NAME       override the profile encoder: x264, vaapi, vp9
  --no-cursor          do not draw the mouse cursor in the video

options can also be set in ~/.config/i3snip/config as key=value lines
(dim=40, border-color=9aac6f, audio=default, ...); flags win over the file.
";

fn home() -> Res<PathBuf> {
    Ok(PathBuf::from(env::var_os("HOME").ok_or("HOME is not set")?))
}

pub fn parse_area(s: &str) -> Res<Rect> {
    let v: Vec<i64> = s.split(',').map(|p| p.trim().parse()).collect::<Result<_, _>>()?;
    if v.len() != 4 || v[2] <= 0 || v[3] <= 0 {
        return Err("--area expects X,Y,W,H with positive size".into());
    }
    Ok(Rect {
        x: v[0] as i32,
        y: v[1] as i32,
        w: v[2] as u32,
        h: v[3] as u32,
    })
}

fn parse_color(s: &str) -> Res<u32> {
    let s = s.trim_start_matches('#');
    if s.len() != 6 {
        return Err(format!("bad color '{s}', expected 6 hex digits").into());
    }
    Ok(u32::from_str_radix(s, 16)?)
}

fn expand(p: &str) -> Res<PathBuf> {
    match p.strip_prefix("~/") {
        Some(rest) => Ok(home()?.join(rest)),
        None => Ok(PathBuf::from(p)),
    }
}

const DEFAULT_FILE: &str = "dim=45
border-color=9aac6f
border-width=2
monitor=focus
profile=balanced
pictures-dir=~/Pictures/Screenshots
videos-dir=~/Videos
";

fn config_path() -> Option<PathBuf> {
    let base = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| home().ok().map(|h| h.join(".config")))?;
    Some(base.join("i3snip/config"))
}

fn file_options() -> HashMap<String, String> {
    let mut map = HashMap::new();
    let Some(path) = config_path() else { return map };
    if !path.exists() {
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let _ = fs::write(&path, DEFAULT_FILE);
    }
    let Ok(text) = fs::read_to_string(&path) else {
        return map;
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            map.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    map
}

fn flag_options(args: &[String]) -> Res<HashMap<String, String>> {
    let mut map = HashMap::new();
    let mut i = 0;
    while i < args.len() {
        let key = args[i]
            .strip_prefix("--")
            .ok_or_else(|| format!("unexpected argument '{}'", args[i]))?;
        match key {
            "full" | "no-clipboard" | "no-save" | "no-notify" | "no-cursor" => {
                map.insert(key.to_string(), "true".to_string());
            }
            "audio" => {
                let next = args.get(i + 1).filter(|a| !a.starts_with("--"));
                map.insert(key.to_string(), next.cloned().unwrap_or_else(|| "default".into()));
                if next.is_some() {
                    i += 1;
                }
            }
            "area" | "delay" | "dim" | "border-color" | "border-width" | "pictures-dir" | "videos-dir" | "fps"
            | "crf" | "preset" | "ext" | "encoder" | "monitor" | "profile" => {
                i += 1;
                let v = args.get(i).ok_or_else(|| format!("--{key} needs a value"))?;
                map.insert(key.to_string(), v.clone());
            }
            _ => return Err(format!("unknown option --{key}").into()),
        }
        i += 1;
    }
    Ok(map)
}

pub fn load(args: &[String]) -> Res<Config> {
    let mode = match args.first().map(String::as_str) {
        Some("photo") => Mode::Photo,
        Some("video") => Mode::Video,
        Some("stop") => Mode::Stop,
        _ => return Err("missing or unknown mode".into()),
    };
    let mut opts = file_options();
    opts.extend(flag_options(&args[1..])?);

    let flag = |k: &str| opts.get(k).map(|v| v == "true").unwrap_or(false);
    let num = |k: &str, d: u64| -> Res<u64> { Ok(opts.get(k).map(|v| v.parse()).transpose()?.unwrap_or(d)) };
    let dim = num("dim", 45)?.min(100) as u32;
    let profile = opts.get("profile").map(String::as_str).unwrap_or("balanced");
    let hw = Path::new("/dev/dri/renderD128").exists();
    let (p_encoder, p_preset, p_crf, p_fps, tune) = match profile {
        "light" => (
            if hw { "vaapi" } else { "x264" },
            "superfast",
            28,
            60,
            Some("zerolatency"),
        ),
        "balanced" => ("x264", "veryfast", 23, 60, Some("zerolatency")),
        "quality" => ("x264", "medium", 18, 60, None),
        other => return Err(format!("unknown profile '{other}'").into()),
    };
    let ext = opts.get("ext").map(String::as_str).unwrap_or("mp4");
    if !["mp4", "mkv", "webm"].contains(&ext) {
        return Err(format!("unsupported container '{ext}', use mp4, mkv or webm").into());
    }
    let encoder = opts.get("encoder").map(String::as_str).unwrap_or(p_encoder).to_string();
    if !["x264", "vaapi", "vp9"].contains(&encoder.as_str()) {
        return Err(format!("unknown encoder '{encoder}'").into());
    }

    Ok(Config {
        mode,
        area: opts.get("area").map(|s| parse_area(s)).transpose()?,
        full: flag("full"),
        delay: num("delay", 0)?,
        dim,
        border_color: parse_color(opts.get("border-color").map(String::as_str).unwrap_or("9aac6f"))?,
        border_width: num("border-width", 2)?.clamp(1, 20) as u16,
        pictures_dir: match opts.get("pictures-dir") {
            Some(p) => expand(p)?,
            None => home()?.join("Pictures/Screenshots"),
        },
        videos_dir: match opts.get("videos-dir") {
            Some(p) => expand(p)?,
            None => home()?.join("Videos"),
        },
        clipboard: !flag("no-clipboard"),
        save: !flag("no-save"),
        notify: !flag("no-notify"),
        fps: num("fps", p_fps)?.max(1) as u32,
        crf: num("crf", p_crf)?.min(51) as u32,
        preset: opts.get("preset").cloned().unwrap_or_else(|| p_preset.into()),
        tune,
        audio: opts.get("audio").cloned(),
        ext: ext.to_string(),
        encoder,
        monitor: opts.get("monitor").cloned().unwrap_or_else(|| "focus".into()),
        cursor: !flag("no-cursor"),
    })
}
