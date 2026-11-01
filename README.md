# i3snip
Screenshots and screen recordings for i3 and other X11 window managers.
```
i3snip photo    copy a region to the clipboard and save a PNG
i3snip video    record a region with ffmpeg, run again to stop
i3snip stop     stop a recording
```

## Install
```
cargo install --path . --root ~/.local
```
Arch: `cd packaging/arch && makepkg -si`. Debian, Ubuntu and Fedora packages are on the releases page. Needs `xclip` and `ffmpeg`.

Configuration for i3:
```
bindsym $mod+Shift+s exec --no-startup-id ~/.local/bin/i3snip photo
bindsym $mod+Shift+r exec --no-startup-id ~/.local/bin/i3snip video
```

## Options

| Option | Description |
|---|---|
| `--area X,Y,W,H` | skip the selector and use this region |
| `--full` | capture the whole screen of the selected monitor |
| `--monitor WHICH` | `focus` (monitor under the pointer, default), `all`, or an output name like `eDP-1` |
| `--delay SECONDS` | wait before grabbing the screen |
| `--dim PERCENT` | how much to darken outside the selection (default 45) |
| `--border-color HEX` | selection border color (default 9aac6f) |
| `--border-width PX` | selection border width (default 2) |
| `--pictures-dir DIR` | default `~/Pictures/Screenshots` |
| `--videos-dir DIR` | default `~/Videos` |
| `--no-clipboard` | do not copy the photo or the video file to the clipboard |
| `--no-save` | do not write photos to disk |
| `--no-notify` | no notifications |
| `--profile NAME` | `light` (hardware encoder when available), `balanced` (default), `quality` |
| `--fps N` | override the profile frame rate (60 by default) |
| `--crf N` | override the quality, lower is better (0-51) |
| `--preset NAME` | override the x264 preset |
| `--audio [SOURCE]` | record audio (default source when omitted) |
| `--ext EXT` | container: `mp4` (default), `mkv`, `webm` |
| `--encoder NAME` | override the profile encoder: `x264`, `vaapi`, `vp9` |
| `--no-cursor` | do not draw the mouse cursor in the video |

Options can also be set in `~/.config/i3snip/config` as `key=value` lines.
