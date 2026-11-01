use crate::config::{Rect, Res};
use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::randr;
use x11rb::protocol::xproto::*;
use x11rb::rust_connection::RustConnection;

pub struct Shot {
    pub w: u32,
    pub h: u32,
    pub ox: i32,
    pub oy: i32,
    pub data: Vec<u8>,
}

pub struct Area {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

pub fn clamp(r: Rect, sw: u32, sh: u32) -> Option<Rect> {
    let x0 = r.x.max(0);
    let y0 = r.y.max(0);
    let x1 = (r.x + r.w as i32).min(sw as i32);
    let y1 = (r.y + r.h as i32).min(sh as i32);
    (x1 > x0 && y1 > y0).then(|| Rect {
        x: x0,
        y: y0,
        w: (x1 - x0) as u32,
        h: (y1 - y0) as u32,
    })
}

fn monitors(conn: &RustConnection, root: Window) -> Vec<Area> {
    let Ok(cookie) = randr::get_monitors(conn, root, true) else {
        return Vec::new();
    };
    let Ok(reply) = cookie.reply() else { return Vec::new() };
    reply
        .monitors
        .iter()
        .filter(|m| m.width > 0 && m.height > 0)
        .map(|m| {
            let name = conn
                .get_atom_name(m.name)
                .ok()
                .and_then(|c| c.reply().ok())
                .map(|r| String::from_utf8_lossy(&r.name).into_owned())
                .unwrap_or_default();
            Area {
                name,
                x: m.x as i32,
                y: m.y as i32,
                w: m.width as u32,
                h: m.height as u32,
            }
        })
        .collect()
}

pub fn choose_area(conn: &RustConnection, screen_num: usize, which: &str) -> Res<Area> {
    let screen = &conn.setup().roots[screen_num];
    let whole = Area {
        name: "all".into(),
        x: 0,
        y: 0,
        w: screen.width_in_pixels as u32,
        h: screen.height_in_pixels as u32,
    };
    if which == "all" {
        return Ok(whole);
    }
    let mut list = monitors(conn, screen.root);
    if list.is_empty() {
        return Ok(whole);
    }
    if which != "focus" {
        return list
            .into_iter()
            .find(|m| m.name == which)
            .ok_or_else(|| format!("no monitor named '{which}'").into());
    }
    let pointer = conn.query_pointer(screen.root)?.reply()?;
    let (px, py) = (pointer.root_x as i32, pointer.root_y as i32);
    let idx = list
        .iter()
        .position(|m| px >= m.x && px < m.x + m.w as i32 && py >= m.y && py < m.y + m.h as i32)
        .unwrap_or(0);
    Ok(list.swap_remove(idx))
}

pub fn grab(conn: &RustConnection, screen_num: usize, a: &Area) -> Res<Shot> {
    let screen = &conn.setup().roots[screen_num];
    let fmt = conn
        .setup()
        .pixmap_formats
        .iter()
        .find(|f| f.depth == screen.root_depth)
        .ok_or("no matching pixmap format")?;
    if fmt.bits_per_pixel != 32 {
        return Err(format!("unsupported pixel format: {} bpp", fmt.bits_per_pixel).into());
    }
    let reply = conn
        .get_image(
            ImageFormat::Z_PIXMAP,
            screen.root,
            a.x as i16,
            a.y as i16,
            a.w as u16,
            a.h as u16,
            !0,
        )?
        .reply()?;
    Ok(Shot {
        w: a.w,
        h: a.h,
        ox: a.x,
        oy: a.y,
        data: reply.data,
    })
}

pub fn upload(conn: &RustConnection, pm: Pixmap, gc: Gcontext, depth: u8, w: u32, data: &[u8]) -> Res<()> {
    let row_bytes = w as usize * 4;
    let max = conn.maximum_request_bytes().saturating_sub(64);
    let rows = (max / row_bytes).max(1) as u32;
    let total = (data.len() / row_bytes) as u32;
    let mut y = 0;
    while y < total {
        let n = rows.min(total - y);
        let start = y as usize * row_bytes;
        let end = start + n as usize * row_bytes;
        conn.put_image(
            ImageFormat::Z_PIXMAP,
            pm,
            gc,
            w as u16,
            n as u16,
            0,
            y as i16,
            0,
            depth,
            &data[start..end],
        )?;
        y += n;
    }
    Ok(())
}
