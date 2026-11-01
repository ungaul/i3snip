use crate::capture::{clamp, upload, Shot};
use crate::config::{Config, Rect, Res};
use std::thread::sleep;
use std::time::Duration;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::{COPY_DEPTH_FROM_PARENT, CURRENT_TIME};

const KEY_ESCAPE: u8 = 9;
const CROSSHAIR: u16 = 34;
const REPAINT_MARGIN: i32 = 4;

fn normalise(x0: i32, y0: i32, x1: i32, y1: i32) -> Rect {
    Rect {
        x: x0.min(x1),
        y: y0.min(y1),
        w: (x0 - x1).unsigned_abs() + 1,
        h: (y0 - y1).unsigned_abs() + 1,
    }
}

struct Layers {
    win: Window,
    orig: Pixmap,
    dim: Pixmap,
    gc: Gcontext,
    line: Gcontext,
}

fn repaint(conn: &RustConnection, l: &Layers, old: Option<Rect>, new: Rect) -> Res<()> {
    let m = REPAINT_MARGIN;
    let (x0, y0, x1, y1) = match old {
        Some(o) => (
            o.x.min(new.x) - m,
            o.y.min(new.y) - m,
            (o.x + o.w as i32).max(new.x + new.w as i32) + m,
            (o.y + o.h as i32).max(new.y + new.h as i32) + m,
        ),
        None => (new.x - m, new.y - m, new.x + new.w as i32 + m, new.y + new.h as i32 + m),
    };
    let (x0, y0) = (x0.max(0), y0.max(0));
    conn.copy_area(
        l.dim,
        l.win,
        l.gc,
        x0 as i16,
        y0 as i16,
        x0 as i16,
        y0 as i16,
        (x1 - x0) as u16,
        (y1 - y0) as u16,
    )?;
    conn.copy_area(
        l.orig,
        l.win,
        l.gc,
        new.x as i16,
        new.y as i16,
        new.x as i16,
        new.y as i16,
        new.w as u16,
        new.h as u16,
    )?;
    conn.poly_rectangle(
        l.win,
        l.line,
        &[Rectangle {
            x: new.x as i16,
            y: new.y as i16,
            width: new.w.saturating_sub(1) as u16,
            height: new.h.saturating_sub(1) as u16,
        }],
    )?;
    Ok(())
}

fn grab_input(conn: &RustConnection, win: Window, cursor: Cursor) -> Res<()> {
    let mask = EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE | EventMask::POINTER_MOTION;
    for _ in 0..50 {
        let p = conn
            .grab_pointer(
                true,
                win,
                mask,
                GrabMode::ASYNC,
                GrabMode::ASYNC,
                win,
                cursor,
                CURRENT_TIME,
            )?
            .reply()?;
        let k = conn
            .grab_keyboard(true, win, CURRENT_TIME, GrabMode::ASYNC, GrabMode::ASYNC)?
            .reply()?;
        if p.status == GrabStatus::SUCCESS && k.status == GrabStatus::SUCCESS {
            return Ok(());
        }
        let _ = conn.ungrab_pointer(CURRENT_TIME);
        let _ = conn.ungrab_keyboard(CURRENT_TIME);
        conn.flush()?;
        sleep(Duration::from_millis(20));
    }
    Err("could not grab the pointer and keyboard".into())
}

pub fn select(conn: &RustConnection, screen_num: usize, shot: &Shot, cfg: &Config) -> Res<Option<Rect>> {
    let screen = &conn.setup().roots[screen_num];
    let (w, h, depth) = (shot.w as u16, shot.h as u16, screen.root_depth);
    let keep = 100 - cfg.dim;
    let dimmed: Vec<u8> = shot.data.iter().map(|&v| ((v as u32 * keep) / 100) as u8).collect();

    let l = Layers {
        win: conn.generate_id()?,
        orig: conn.generate_id()?,
        dim: conn.generate_id()?,
        gc: conn.generate_id()?,
        line: conn.generate_id()?,
    };
    conn.create_pixmap(depth, l.orig, screen.root, w, h)?;
    conn.create_pixmap(depth, l.dim, screen.root, w, h)?;
    conn.create_gc(l.gc, screen.root, &CreateGCAux::new())?;
    upload(conn, l.orig, l.gc, depth, shot.w, &shot.data)?;
    upload(conn, l.dim, l.gc, depth, shot.w, &dimmed)?;

    let font = conn.generate_id()?;
    conn.open_font(font, b"cursor")?;
    let cursor = conn.generate_id()?;
    conn.create_glyph_cursor(
        cursor,
        font,
        font,
        CROSSHAIR,
        CROSSHAIR + 1,
        0xffff,
        0xffff,
        0xffff,
        0,
        0,
        0,
    )?;

    let events = EventMask::EXPOSURE
        | EventMask::BUTTON_PRESS
        | EventMask::BUTTON_RELEASE
        | EventMask::POINTER_MOTION
        | EventMask::KEY_PRESS;
    conn.create_window(
        COPY_DEPTH_FROM_PARENT,
        l.win,
        screen.root,
        shot.ox as i16,
        shot.oy as i16,
        w,
        h,
        0,
        WindowClass::INPUT_OUTPUT,
        0,
        &CreateWindowAux::new()
            .override_redirect(1)
            .cursor(cursor)
            .event_mask(events),
    )?;
    conn.create_gc(
        l.line,
        l.win,
        &CreateGCAux::new()
            .foreground(cfg.border_color)
            .line_width(cfg.border_width as u32),
    )?;
    conn.map_window(l.win)?;
    conn.copy_area(l.dim, l.win, l.gc, 0, 0, 0, 0, w, h)?;
    conn.flush()?;

    let outcome = grab_input(conn, l.win, cursor).and_then(|_| event_loop(conn, &l, shot, w, h));

    let _ = conn.ungrab_pointer(CURRENT_TIME);
    let _ = conn.ungrab_keyboard(CURRENT_TIME);
    conn.destroy_window(l.win)?;
    conn.free_pixmap(l.orig)?;
    conn.free_pixmap(l.dim)?;
    conn.free_cursor(cursor)?;
    conn.close_font(font)?;
    conn.flush()?;
    outcome
}

fn event_loop(conn: &RustConnection, l: &Layers, shot: &Shot, w: u16, h: u16) -> Res<Option<Rect>> {
    let mut anchor: Option<(i32, i32)> = None;
    let mut shown: Option<Rect> = None;
    loop {
        match conn.wait_for_event()? {
            Event::Expose(_) => {
                conn.copy_area(l.dim, l.win, l.gc, 0, 0, 0, 0, w, h)?;
                if let Some(r) = shown {
                    repaint(conn, l, None, r)?;
                }
            }
            Event::ButtonPress(e) if e.detail == 1 => anchor = Some((e.event_x as i32, e.event_y as i32)),
            Event::ButtonPress(e) if e.detail == 2 || e.detail == 3 => return Ok(None),
            Event::MotionNotify(e) => {
                if let Some((ax, ay)) = anchor {
                    let r = normalise(ax, ay, e.event_x as i32, e.event_y as i32);
                    repaint(conn, l, shown, r)?;
                    shown = Some(r);
                }
            }
            Event::ButtonRelease(e) if e.detail == 1 => {
                let Some((ax, ay)) = anchor else { continue };
                let r = normalise(ax, ay, e.event_x as i32, e.event_y as i32);
                return Ok(if r.w >= 4 && r.h >= 4 {
                    clamp(r, shot.w, shot.h)
                } else {
                    None
                });
            }
            Event::KeyPress(e) if e.detail == KEY_ESCAPE => return Ok(None),
            _ => {}
        }
        conn.flush()?;
    }
}
