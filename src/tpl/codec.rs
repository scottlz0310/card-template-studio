//! オブジェクトの読み書き（本体の並びごと）。

use super::body::{Body, CaptionBody, CmValueBody, PictureBody};
use super::error::{ParseError, Result};
use super::io::{Reader, Writer};
use super::object::Object;
use super::shape::skip_passthrough_body;
use super::{
    KIND_CAPTION, KIND_CM_VALUE, KIND_DRAFT, KIND_DRAW_OBJECT, KIND_PATTERN, KIND_PICTURE,
    KIND_POLY, KIND_RECT,
};

pub(super) fn read_object(r: &mut Reader) -> Result<Object> {
    let start = r.pos;
    let tag = r.tag()?;
    if &tag[..3] != b"OBJ" || !(b'1'..=b'3').contains(&tag[3]) {
        return Err(ParseError {
            offset: start,
            message: "OBJ ヘッダが見つかりません".into(),
        });
    }
    let tag_version = tag[3] - b'0';
    let kind = r.u32()?;
    let link = r.u32()?;
    let rect = [r.f64()?, r.f64()?, r.f64()?, r.f64()?];
    let body = match kind {
        KIND_PICTURE => Body::Picture(read_picture(r, tag_version)?),
        KIND_CAPTION => Body::Caption(read_caption(r, tag_version)?),
        KIND_CM_VALUE => Body::CmValue(read_cm_value(r, tag_version)?),
        KIND_DRAW_OBJECT | KIND_DRAFT | KIND_RECT | KIND_PATTERN | KIND_POLY => {
            // 長さを知るためだけに本体の並び（0x426840 / 0x41d320 / 0x421640 / 0x421870 / 0x421b00）を辿る
            let body_start = r.pos;
            skip_passthrough_body(r, kind)?;
            Body::Passthrough(r.data[body_start..r.pos].to_vec())
        }
        other => {
            return Err(ParseError {
                offset: start + 4,
                message: format!("未知の種類 {other}"),
            });
        }
    };
    let trailer = if tag_version >= 3 {
        Some(r.u32()?)
    } else {
        None
    };
    Ok(Object {
        tag_version,
        kind,
        link,
        rect,
        body,
        trailer,
    })
}

pub(super) fn write_object(w: &mut Writer, o: &Object) {
    w.bytes(b"OBJ");
    w.u8(b'0' + o.tag_version.clamp(1, 3));
    w.u32(o.kind);
    w.u32(o.link);
    for v in o.rect {
        w.f64(v);
    }
    match &o.body {
        Body::Picture(p) => write_picture(w, p),
        Body::Caption(c) => write_caption(w, c),
        Body::CmValue(c) => write_cm_value(w, c),
        Body::Passthrough(raw) => w.bytes(raw),
    }
    if o.tag_version >= 3 {
        w.u32(o.trailer.unwrap_or(0));
    }
}

fn read_picture(r: &mut Reader, tag_version: u8) -> Result<PictureBody> {
    Ok(PictureBody {
        crop: r.array()?,
        frame_style: r.u32()?,
        border_color: r.u32()?,
        guid: r.cstring()?,
        c4: r.cstring()?,
        path: r.cstring()?,
        cc: r.u32()?,
        d0: r.u32()?,
        d4: r.u32()?,
        d8: r.u32()?,
        dc: r.cstring()?,
        e0: r.u32()?,
        e4: r.cstring()?,
        e8: r.u32()?,
        // 0x42c060: 版（タグの数字）が 1 より大きいときだけ +0xec を読む
        ec: if tag_version > 1 {
            Some(r.u32()?)
        } else {
            None
        },
    })
}

fn write_picture(w: &mut Writer, p: &PictureBody) {
    w.bytes(&p.crop);
    w.u32(p.frame_style);
    w.u32(p.border_color);
    w.cstring(&p.guid);
    w.cstring(&p.c4);
    w.cstring(&p.path);
    w.u32(p.cc);
    w.u32(p.d0);
    w.u32(p.d4);
    w.u32(p.d8);
    w.cstring(&p.dc);
    w.u32(p.e0);
    w.cstring(&p.e4);
    w.u32(p.e8);
    if let Some(v) = p.ec {
        w.u32(v);
    }
}

fn read_caption(r: &mut Reader, tag_version: u8) -> Result<CaptionBody> {
    let text = r.cstring()?;
    let face = r.array()?;
    let fmt = r.array()?;
    let value = r.cstring()?;
    let field = r.cstring()?;
    let t108 = r.i32()?;
    let is_value = r.i32()?;
    let border = r.i32()?;
    let border_inset = r.i32()?;
    // 0x408610: +0x114 が 0..2 で、版が 1 より大きいときだけ続きの 2 つを読む
    let border_style = if (0..=2).contains(&border_inset) && tag_version > 1 {
        Some((r.u32()?, r.i32()?))
    } else {
        None
    };
    Ok(CaptionBody {
        text,
        face,
        fmt,
        value,
        field,
        t108,
        is_value,
        border,
        border_inset,
        border_style,
    })
}

fn write_caption(w: &mut Writer, c: &CaptionBody) {
    w.cstring(&c.text);
    w.bytes(&c.face);
    w.bytes(&c.fmt);
    w.cstring(&c.value);
    w.cstring(&c.field);
    w.i32(c.t108);
    w.i32(c.is_value);
    w.i32(c.border);
    w.i32(c.border_inset);
    if let Some((color, width)) = c.border_style {
        w.u32(color);
        w.i32(width);
    }
}

fn read_cm_value(r: &mut Reader, tag_version: u8) -> Result<CmValueBody> {
    let text = r.cstring()?;
    let c0 = r.u32()?;
    let count = r.u32()?;
    let design_color = r.u32()?;
    let actual_color = r.u32()?;
    let border_color = r.u32()?;
    let face = r.array()?;
    let fmt = r.array()?;
    let layout = r.u32()?;
    let name_style = r.u32()?;
    let has_border = r.u32()?;
    let border_weight = r.u32()?;
    let x144 = r.u32()?;
    let ext = if tag_version > 1 {
        let a = r.u32()?;
        let b = r.u32()?;
        let n = r.u32()? as usize;
        if n > 10_000 {
            return Err(r.err("施工管理値の行数が大きすぎます"));
        }
        let mut rows = Vec::with_capacity(n);
        for _ in 0..n {
            rows.push(r.array()?);
        }
        Some((a, b, rows))
    } else {
        None
    };
    Ok(CmValueBody {
        text,
        c0,
        count,
        design_color,
        actual_color,
        border_color,
        face,
        fmt,
        layout,
        name_style,
        has_border,
        border_weight,
        x144,
        ext,
    })
}

fn write_cm_value(w: &mut Writer, c: &CmValueBody) {
    w.cstring(&c.text);
    for v in [
        c.c0,
        c.count,
        c.design_color,
        c.actual_color,
        c.border_color,
    ] {
        w.u32(v);
    }
    w.bytes(&c.face);
    w.bytes(&c.fmt);
    for v in [
        c.layout,
        c.name_style,
        c.has_border,
        c.border_weight,
        c.x144,
    ] {
        w.u32(v);
    }
    if let Some((a, b, rows)) = &c.ext {
        w.u32(*a);
        w.u32(*b);
        w.u32(rows.len() as u32);
        for row in rows {
            w.bytes(row);
        }
    }
}
