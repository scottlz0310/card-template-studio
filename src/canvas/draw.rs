//! 描画（カード・部品・定規）。

use super::view::{View, Xf};
use super::{ACCENT, PHOTO_EDGE, PHOTO_FILL, RULER};
use crate::layout::CardFrame;
use crate::tpl::{self, Body, MM_PER_INCH, Object, ShapeKind, fmt_off, read_fmt, read_fmt_u16};
use egui::{
    Align2, Color32, CornerRadius, FontFamily, FontId, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2,
    pos2, text::LayoutJob, vec2,
};

pub fn color(c: u32) -> Color32 {
    let [r, g, b] = tpl::colorref_rgb(c);
    Color32::from_rgb(r, g, b)
}

pub fn font_family(face: &str) -> FontFamily {
    let key = match face {
        f if f.contains("メイリオ") || f.to_ascii_lowercase().contains("meiryo") => "meiryo",
        f if f.contains("明朝") && (f.contains('Ｐ') || f.contains('P')) => "pmincho",
        f if f.contains("明朝") => "mincho",
        f if f.contains("ゴシック") && !(f.contains('Ｐ') || f.contains('P')) => "gothic",
        _ => "pgothic",
    };
    FontFamily::Name(key.into())
}

/// 項目値の見本文字
pub(super) fn sample_value(field: &str) -> &'static str {
    match field {
        "写真タイトル" => "床掘状況",
        "撮影箇所" => "No.12+5.0",
        "撮影年月日" => "2026/11/09",
        "工種" => "道路土工",
        "種別" => "掘削工",
        "細別" => "掘削",
        "請負者説明文" => "設計通り施工",
        "施工管理値" => "規格値 ±50 / 実測 +12",
        _ => "（データ）",
    }
}

pub(super) fn draw_card(
    p: &egui::Painter,
    xf: &Xf,
    frame: &CardFrame,
    view: &View,
    card_color: u32,
) {
    let card = xf.rect([0.0, 0.0, frame.width, frame.height]);
    p.rect_filled(
        card.translate(vec2(2.0, 3.0)),
        3.0,
        Color32::from_black_alpha(40),
    );
    p.rect_filled(card, 0.0, color(card_color));
    if view.show_grid {
        let step_mm = if xf.ppi / MM_PER_INCH as f32 * view.grid_mm < 5.0 {
            5.0
        } else {
            view.grid_mm
        };
        let step = step_mm as f64 / MM_PER_INCH;
        let mut k = 1;
        loop {
            let v = k as f64 * step;
            if v >= frame.width.max(frame.height) {
                break;
            }
            let strong = ((k as f32 * step_mm) % 10.0).abs() < 1e-3;
            let c = if strong {
                Color32::from_rgba_unmultiplied(80, 110, 160, 46)
            } else {
                Color32::from_rgba_unmultiplied(80, 110, 160, 18)
            };
            if v < frame.width {
                let x = xf.pt(v, 0.0).x;
                p.line_segment(
                    [pos2(x, card.top()), pos2(x, card.bottom())],
                    Stroke::new(1.0, c),
                );
            }
            if v < frame.height {
                let y = xf.pt(0.0, v).y;
                p.line_segment(
                    [pos2(card.left(), y), pos2(card.right(), y)],
                    Stroke::new(1.0, c),
                );
            }
            k += 1;
        }
    }
    if view.show_used
        && let Some(u) = frame.used
    {
        let r = xf.rect(u);
        p.extend(Shape::dashed_line(
            &[
                r.left_top(),
                r.right_top(),
                r.right_bottom(),
                r.left_bottom(),
                r.left_top(),
            ],
            Stroke::new(1.0, Color32::from_rgb(0x4c, 0xa3, 0x7a)),
            6.0,
            4.0,
        ));
    }
    p.rect_stroke(
        card,
        0.0,
        Stroke::new(1.0, Color32::from_gray(150)),
        StrokeKind::Outside,
    );
}

/// 写真枠の太さ（なし / 細い / 中 / 太い）を、画面に描く線の太さ（mm）にしたもの。
/// 太さの順序は実機で確認済み。mm の値は実機の見た目から目視で見積もった推定
const PICTURE_BORDER_MM: [f64; 4] = [0.0, 0.3, 0.6, 1.0];

/// 角丸の半径の、枠の短い辺に対する割合（実機の見た目からの推定）
const PICTURE_CORNER_RATIO: f32 = 0.16;

/// 写真枠の下地と枠線（形・太さ・色は `PictureBody` の値）
fn draw_picture_frame(p: &egui::Painter, xf: &Xf, r: Rect, pb: &tpl::PictureBody) {
    let stroke = match pb.border() {
        // 本体は枠線なしだが、編集中に枠を見失わないよう、うっすらと縁を描く
        tpl::PictureBorder::None => Stroke::new(1.0, PHOTO_EDGE.gamma_multiply(0.4)),
        b => {
            let mm = PICTURE_BORDER_MM[b as usize];
            let px = (mm / MM_PER_INCH * xf.ppi as f64).max(1.0) as f32;
            Stroke::new(px, color(pb.border_color))
        }
    };
    match pb.shape() {
        tpl::PictureShape::Rect => {
            p.rect(r, 0.0, PHOTO_FILL, stroke, StrokeKind::Inside);
        }
        tpl::PictureShape::Rounded => {
            let radius = (r.width().min(r.height()) * PICTURE_CORNER_RATIO).round();
            p.rect(r, radius, PHOTO_FILL, stroke, StrokeKind::Inside);
        }
        tpl::PictureShape::Ellipse => {
            let radii = r.size() / 2.0 - Vec2::splat(stroke.width / 2.0);
            p.add(Shape::ellipse_filled(r.center(), radii, PHOTO_FILL));
            p.add(Shape::ellipse_stroke(r.center(), radii, stroke));
        }
    }
}

pub(super) fn draw_object(
    p: &egui::Painter,
    xf: &Xf,
    o: &Object,
    picture_no: usize,
    samples: bool,
) {
    let r = xf.rect(o.normalized());
    match &o.body {
        Body::Picture(pb) => {
            draw_picture_frame(p, xf, r, pb);
            // 山と太陽の簡単な絵
            let s = (r.width().min(r.height()) * 0.28).clamp(8.0, 90.0);
            let c = r.center();
            let base = c.y + s * 0.45;
            let mountain = [
                pos2(c.x - s, base),
                pos2(c.x - s * 0.3, c.y - s * 0.35),
                pos2(c.x + s * 0.1, c.y + s * 0.05),
                pos2(c.x + s * 0.45, c.y - s * 0.2),
                pos2(c.x + s, base),
            ];
            p.add(Shape::convex_polygon(
                mountain[..3].to_vec(),
                PHOTO_EDGE.gamma_multiply(0.45),
                Stroke::NONE,
            ));
            p.add(Shape::convex_polygon(
                vec![mountain[2], mountain[3], mountain[4], mountain[0]],
                PHOTO_EDGE.gamma_multiply(0.35),
                Stroke::NONE,
            ));
            p.circle_filled(
                pos2(c.x + s * 0.55, c.y - s * 0.6),
                s * 0.16,
                PHOTO_EDGE.gamma_multiply(0.5),
            );
            if r.height() > 24.0 {
                p.text(
                    r.left_top() + vec2(6.0, 4.0),
                    Align2::LEFT_TOP,
                    format!("写真 {picture_no}"),
                    FontId::proportional(12.0),
                    PHOTO_EDGE,
                );
            }
        }
        Body::Caption(c) => {
            let fmt = &c.fmt;
            if fmt[fmt_off::FILL] != 0 {
                p.rect_filled(r, 0.0, color(read_fmt(fmt, fmt_off::BG_COLOR)));
            }
            if c.border != 0 {
                let (col, w) = c.border_style.unwrap_or((0, 0));
                let mm = tpl::BORDER_WIDTHS_MM[(w.clamp(0, 2)) as usize];
                let px = (mm / MM_PER_INCH * xf.ppi as f64).max(1.0) as f32;
                p.rect_stroke(r, 0.0, Stroke::new(px, color(col)), StrokeKind::Inside);
            }
            let field = c.field.to_string_lossy();
            let (text, placeholder) = if c.is_value_cell() {
                if samples {
                    (sample_value(&field).to_string(), false)
                } else {
                    (format!("〈{field}〉"), true)
                }
            } else {
                (c.text.to_string_lossy(), false)
            };
            draw_caption_text(
                p,
                xf,
                r,
                &tpl::fixed_to_string(&c.face),
                fmt,
                &text,
                placeholder,
            );
        }
        Body::CmValue(c) => {
            p.rect(
                r,
                0.0,
                Color32::from_rgb(0xf6, 0xf3, 0xe8),
                Stroke::new(1.0, Color32::from_rgb(0xb0, 0x9a, 0x5a)),
                StrokeKind::Inside,
            );
            let mut label = c.text.to_string_lossy();
            if label.is_empty() || label == "0" {
                label = "施工管理値".into();
            }
            draw_caption_text(
                p,
                xf,
                r,
                &tpl::fixed_to_string(&c.face),
                &c.fmt,
                &format!("▦ {label}"),
                true,
            );
        }
        Body::Passthrough(_) => draw_passthrough(p, xf, o, r),
    }
}

/// 影のずれ。向きは右から反時計回りの度（0 右 / 90 上 / 270 下）。画面は下が正なので y を反転する
pub(super) fn shadow_offset(angle_deg: i32, dist: f32) -> Vec2 {
    let rad = (angle_deg as f32).to_radians();
    vec2(rad.cos(), -rad.sin()) * dist
}

/// 袋文字の縁を作るために、文字を重ねて描くずれの一覧（半径 `w` の円の内側）。
///
/// 円周だけに並べると、細い文字や小さい文字では太さが増すほど元の字形と縁の間に隙間が空く。
/// そこで、同心円状に `spacing`（文字の線の太さ程度）以下の間隔で並べ、隣り合う複製が必ず重なるようにする。
/// 描く数が増えすぎないよう、円の数は `MAX_OUTLINE_RINGS` までで、それを超える太さは
/// その半径までで描く（実機でも、この太さは「大きな塊」になる設定）。
pub(super) fn outline_offsets(w: f32, spacing: f32) -> Vec<Vec2> {
    const MAX_OUTLINE_RINGS: usize = 20;
    const MIN_STEPS: usize = 16;
    if w <= 0.0 || spacing <= 0.0 {
        return Vec::new();
    }
    let w = w.min(spacing * MAX_OUTLINE_RINGS as f32);
    let rings = (w / spacing).ceil().max(1.0) as usize;
    let mut out = Vec::new();
    for k in 1..=rings {
        let r = w * k as f32 / rings as f32;
        let steps = ((std::f32::consts::TAU * r / spacing).ceil() as usize).max(MIN_STEPS);
        // 円ごとに半歩ずらして、隣の円の隙間を埋める
        let phase = if k % 2 == 0 { 0.5 } else { 0.0 };
        for i in 0..steps {
            let rad = (i as f32 + phase) / steps as f32 * std::f32::consts::TAU;
            out.push(vec2(rad.cos(), rad.sin()) * r);
        }
    }
    out
}

pub(super) fn draw_caption_text(
    p: &egui::Painter,
    xf: &Xf,
    r: Rect,
    face: &str,
    fmt: &[u8; tpl::FMT_LEN],
    text: &str,
    placeholder: bool,
) {
    if text.is_empty() {
        return;
    }
    let size_pt = read_fmt_u16(fmt, fmt_off::SIZE).max(1) as f32;
    let px = (size_pt / 72.0 * xf.ppi).max(2.0);
    let family = font_family(face);
    let col = if placeholder {
        Color32::from_rgb(0x8a, 0x93, 0xa3)
    } else {
        color(read_fmt(fmt, fmt_off::TEXT_COLOR))
    };
    let bold = read_fmt(fmt, fmt_off::WEIGHT) as i32 > 400;
    let italic = fmt[fmt_off::ITALIC] != 0 || placeholder;
    let underline = fmt[fmt_off::UNDERLINE] != 0;
    let strike = fmt[fmt_off::STRIKE] != 0;
    let vertical = fmt[fmt_off::HORIZONTAL] == 0;
    let wrap = fmt[fmt_off::WRAP] != 0;
    let align = fmt[fmt_off::ALIGN];
    let line_pct = read_fmt_u16(fmt, fmt_off::LINE_SPACE) as f32;
    let char_pct = read_fmt_u16(fmt, fmt_off::CHAR_SPACE) as f32;
    let pad = (0.5 / MM_PER_INCH as f32 * xf.ppi).max(1.0);
    let inner = r.shrink(pad);
    let clip = p.with_clip_rect(r.intersect(p.clip_rect()));

    let shown: String = if vertical {
        text.chars().flat_map(|c| [c, '\n']).collect()
    } else {
        text.to_string()
    };
    // 影・縁取りは文字色だけ違う同じ配置を重ねて描くので、色を引数に配置を作る
    let layout = |col: Color32| {
        let mut job = LayoutJob::default();
        let tf = egui::TextFormat {
            font_id: FontId::new(px, family.clone()),
            color: col,
            italics: italic,
            underline: if underline {
                Stroke::new((px / 14.0).max(1.0), col)
            } else {
                Stroke::NONE
            },
            strikethrough: if strike {
                Stroke::new((px / 14.0).max(1.0), col)
            } else {
                Stroke::NONE
            },
            extra_letter_spacing: px * char_pct / 100.0,
            line_height: Some(px * (1.0 + line_pct / 100.0) * 1.05),
            ..Default::default()
        };
        job.append(&shown, 0.0, tf);
        job.wrap.max_width = if wrap && !vertical {
            inner.width().max(1.0)
        } else {
            f32::INFINITY
        };
        job.halign = match align {
            1 => egui::Align::Center,
            2 => egui::Align::RIGHT,
            _ => egui::Align::LEFT,
        };
        clip.layout_job(job)
    };
    let galley = layout(col);
    let size = galley.size();
    let x = match (vertical, align) {
        (true, _) => inner.center().x - size.x / 2.0,
        (false, 1) => inner.center().x,
        (false, 2) => inner.right(),
        _ => inner.left(),
    };
    let y = if vertical {
        inner.top()
    } else {
        inner.center().y - size.y / 2.0
    };
    let pos = pos2(x, y.max(inner.top() - 1.0).min(inner.bottom()));
    let paint = |at: Pos2, g: &std::sync::Arc<egui::Galley>| {
        clip.galley(at, g.clone(), col);
        if bold {
            clip.galley(at + vec2((px / 28.0).max(0.6), 0.0), g.clone(), col);
        }
    };
    // 距離・太さは文字の大きさに比例する（絶対の長さではない）
    let effect_px = |units: u16| units as f32 * tpl::EFFECT_UNIT_EM * px;
    match tpl::TextEffect::from_byte(fmt[fmt_off::EFFECT]) {
        tpl::TextEffect::None => {}
        tpl::TextEffect::Shadow => {
            let dist = effect_px(read_fmt_u16(fmt, fmt_off::SHADOW_DISTANCE));
            let angle = read_fmt(fmt, fmt_off::SHADOW_ANGLE) as i32;
            let shadow = layout(color(read_fmt(fmt, fmt_off::SHADOW_COLOR)));
            paint(pos + shadow_offset(angle, dist), &shadow);
        }
        tpl::TextEffect::Outline => {
            let w = effect_px(read_fmt_u16(fmt, fmt_off::OUTLINE_WIDTH));
            let edge = layout(color(read_fmt(fmt, fmt_off::OUTLINE_COLOR)));
            // 文字の線の太さ（文字の大きさの 1 割ほど）より細かい間隔で、縁の円の内側を埋める
            for off in outline_offsets(w, (px * 0.1).max(0.4)) {
                paint(pos + off, &edge);
            }
        }
    }
    paint(pos, &galley);
}

pub(super) fn draw_passthrough(p: &egui::Painter, xf: &Xf, o: &Object, r: Rect) {
    let lock_col = Color32::from_rgb(0x7a, 0x7f, 0x88);
    if let Some(v) = o.shape_view() {
        let pen = if v.base.pen != 0 {
            Stroke::new(
                (v.base.pen_width.max(1) as f32 / 72.0 * xf.ppi).max(1.0),
                color(v.base.pen_color),
            )
        } else {
            Stroke::NONE
        };
        let fill = if v.base.brush != 0 {
            color(v.base.brush_color)
        } else {
            Color32::TRANSPARENT
        };
        let [l, t, rr, b] = o.rect;
        match v.shape {
            ShapeKind::Rectangle => {
                p.rect(r, 0.0, fill, pen, StrokeKind::Inside);
            }
            ShapeKind::RoundRect => {
                let rad = (r.width() * v.round.0 as f32).min(r.height() * v.round.1 as f32) / 2.0;
                p.rect(
                    r,
                    CornerRadius::same(rad.clamp(0.0, 120.0) as u8),
                    fill,
                    pen,
                    StrokeKind::Inside,
                );
            }
            ShapeKind::Ellipse => {
                p.add(Shape::ellipse_filled(r.center(), r.size() / 2.0, fill));
                p.add(Shape::ellipse_stroke(r.center(), r.size() / 2.0, pen));
            }
            ShapeKind::Line | ShapeKind::Arrow => {
                let a = xf.pt(l, t);
                let b = xf.pt(rr, b);
                if v.shape == ShapeKind::Arrow {
                    p.arrow(a, b - a, pen);
                } else {
                    p.line_segment([a, b], pen);
                }
            }
            ShapeKind::Polygon => {
                let pts: Vec<Pos2> = v.points.iter().map(|&(x, y)| xf.pt(x, y)).collect();
                if pts.len() >= 2 {
                    p.add(Shape::closed_line(pts, pen));
                }
            }
        }
    } else {
        p.rect(
            r,
            0.0,
            Color32::from_rgba_unmultiplied(120, 120, 120, 30),
            Stroke::new(1.0, lock_col),
            StrokeKind::Inside,
        );
        p.text(
            r.center(),
            Align2::CENTER_CENTER,
            o.kind_label(),
            FontId::proportional(11.0),
            lock_col,
        );
    }
    p.text(
        r.left_top() + vec2(2.0, 1.0),
        Align2::LEFT_TOP,
        "🔒",
        FontId::proportional(10.0),
        lock_col,
    );
}

pub(super) fn draw_rulers(p: &egui::Painter, area: Rect, xf: &Xf, cursor: Option<(f64, f64)>) {
    let bg = Color32::from_rgb(0xf3, 0xf5, 0xf8);
    let fg = Color32::from_rgb(0x6b, 0x74, 0x80);
    let top = Rect::from_min_max(area.min, pos2(area.right(), area.top() + RULER));
    let left = Rect::from_min_max(area.min, pos2(area.left() + RULER, area.bottom()));
    p.rect_filled(top, 0.0, bg);
    p.rect_filled(left, 0.0, bg);
    let px_per_mm = xf.ppi / MM_PER_INCH as f32;
    let step = if px_per_mm >= 6.0 {
        1
    } else if px_per_mm >= 1.5 {
        5
    } else {
        10
    };
    let label_every = if px_per_mm >= 3.0 { 10 } else { 50 };
    let tp = p.with_clip_rect(Rect::from_min_max(top.min + vec2(RULER, 0.0), top.max));
    let lp = p.with_clip_rect(Rect::from_min_max(left.min + vec2(0.0, RULER), left.max));
    let start_x = (((area.left() + RULER - xf.origin.x) / px_per_mm) as i32 / step - 1) * step;
    let end_x = ((area.right() - xf.origin.x) / px_per_mm) as i32 + step;
    let mut mm = start_x;
    while mm <= end_x {
        let x = xf.origin.x + mm as f32 * px_per_mm;
        let len = if mm % label_every == 0 {
            10.0
        } else if mm % 5 == 0 {
            6.0
        } else {
            3.0
        };
        tp.line_segment(
            [pos2(x, top.bottom() - len), pos2(x, top.bottom())],
            Stroke::new(1.0, fg),
        );
        if mm % label_every == 0 {
            tp.text(
                pos2(x + 2.0, top.top() + 1.0),
                Align2::LEFT_TOP,
                mm.to_string(),
                FontId::proportional(9.5),
                fg,
            );
        }
        mm += step;
    }
    let start_y = (((area.top() + RULER - xf.origin.y) / px_per_mm) as i32 / step - 1) * step;
    let end_y = ((area.bottom() - xf.origin.y) / px_per_mm) as i32 + step;
    let mut mm = start_y;
    while mm <= end_y {
        let y = xf.origin.y + mm as f32 * px_per_mm;
        let len = if mm % label_every == 0 {
            10.0
        } else if mm % 5 == 0 {
            6.0
        } else {
            3.0
        };
        lp.line_segment(
            [pos2(left.right() - len, y), pos2(left.right(), y)],
            Stroke::new(1.0, fg),
        );
        if mm % label_every == 0 {
            lp.text(
                pos2(left.left() + 2.0, y + 2.0),
                Align2::LEFT_TOP,
                mm.to_string(),
                FontId::proportional(9.5),
                fg,
            );
        }
        mm += step;
    }
    if let Some((x, y)) = cursor {
        let sx = xf.pt(x, 0.0).x;
        let sy = xf.pt(0.0, y).y;
        tp.line_segment(
            [pos2(sx, top.top()), pos2(sx, top.bottom())],
            Stroke::new(1.0, ACCENT),
        );
        lp.line_segment(
            [pos2(left.left(), sy), pos2(left.right(), sy)],
            Stroke::new(1.0, ACCENT),
        );
    }
    p.rect_filled(Rect::from_min_size(area.min, Vec2::splat(RULER)), 0.0, bg);
    p.text(
        area.min + Vec2::splat(RULER / 2.0),
        Align2::CENTER_CENTER,
        "mm",
        FontId::proportional(9.0),
        fg,
    );
    p.line_segment(
        [
            pos2(area.left(), top.bottom()),
            pos2(area.right(), top.bottom()),
        ],
        Stroke::new(1.0, Color32::from_gray(200)),
    );
    p.line_segment(
        [
            pos2(left.right(), area.top()),
            pos2(left.right(), area.bottom()),
        ],
        Stroke::new(1.0, Color32::from_gray(200)),
    );
}
