//! カードの編集面：描画、選択、移動、大きさ変更、吸着ガイド、配置ツール。

use crate::doc::Document;
use crate::layout::{CardFrame, card_frame};
use crate::tpl::{self, Body, MM_PER_INCH, Object, ShapeKind, fmt_off, read_fmt, read_fmt_u16};
use egui::{
    Align2, Color32, CornerRadius, FontFamily, FontId, Pos2, Rect, Sense, Shape, Stroke,
    StrokeKind, Vec2, pos2, text::LayoutJob, vec2,
};

/// 1 インチあたりの画面ピクセル（倍率 100%）
pub const PX_PER_INCH: f32 = 96.0;
const RULER: f32 = 22.0;
const HANDLE: f32 = 7.0;
const SNAP_PX: f32 = 7.0;

pub const ACCENT: Color32 = Color32::from_rgb(0x1f, 0x6f, 0xeb);
const GUIDE: Color32 = Color32::from_rgb(0xe0, 0x3a, 0x8c);
const PHOTO_FILL: Color32 = Color32::from_rgb(0xe9, 0xef, 0xf6);
const PHOTO_EDGE: Color32 = Color32::from_rgb(0x8a, 0x9b, 0xb0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Select,
    Picture,
    Label,
    Value,
    Row,
}

impl Tool {
    pub fn label(self) -> &'static str {
        match self {
            Tool::Select => "選択",
            Tool::Picture => "写真枠",
            Tool::Label => "項目名・文字",
            Tool::Value => "項目値",
            Tool::Row => "項目の行（名＋値）",
        }
    }
    pub fn hint(self) -> &'static str {
        match self {
            Tool::Select => {
                "クリックで選択、ドラッグで移動。Shift で追加選択、何もない所をドラッグで範囲選択"
            }
            Tool::Picture => "カード上をドラッグして写真枠を描きます",
            Tool::Label => "ドラッグして固定の文字（項目名など）を置きます",
            Tool::Value => "ドラッグしてデータを表示する欄（項目値）を置きます",
            Tool::Row => "ドラッグして「項目名＋項目値」の 1 行を置きます",
        }
    }
}

pub struct View {
    pub zoom: f32,
    pub pan: Vec2,
    pub fit_pending: bool,
    pub grid_mm: f32,
    pub show_grid: bool,
    pub snap_grid: bool,
    pub snap_objects: bool,
    pub show_used: bool,
    /// 項目値の欄に見本の文字を表示する
    pub sample_values: bool,
}

impl Default for View {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: Vec2::ZERO,
            fit_pending: true,
            grid_mm: 1.0,
            show_grid: true,
            snap_grid: true,
            snap_objects: true,
            show_used: true,
            sample_values: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Handle {
    N,
    S,
    E,
    W,
    NE,
    NW,
    SE,
    SW,
}

impl Handle {
    const ALL: [Handle; 8] = [
        Handle::NW,
        Handle::N,
        Handle::NE,
        Handle::E,
        Handle::SE,
        Handle::S,
        Handle::SW,
        Handle::W,
    ];
    fn pos(self, r: Rect) -> Pos2 {
        let (x, y) = match self {
            Handle::NW => (r.left(), r.top()),
            Handle::N => (r.center().x, r.top()),
            Handle::NE => (r.right(), r.top()),
            Handle::E => (r.right(), r.center().y),
            Handle::SE => (r.right(), r.bottom()),
            Handle::S => (r.center().x, r.bottom()),
            Handle::SW => (r.left(), r.bottom()),
            Handle::W => (r.left(), r.center().y),
        };
        pos2(x, y)
    }
    fn moves(self) -> (bool, bool, bool, bool) {
        // left, top, right, bottom
        match self {
            Handle::N => (false, true, false, false),
            Handle::S => (false, false, false, true),
            Handle::E => (false, false, true, false),
            Handle::W => (true, false, false, false),
            Handle::NE => (false, true, true, false),
            Handle::NW => (true, true, false, false),
            Handle::SE => (false, false, true, true),
            Handle::SW => (true, false, false, true),
        }
    }
    fn cursor(self) -> egui::CursorIcon {
        match self {
            Handle::N | Handle::S => egui::CursorIcon::ResizeVertical,
            Handle::E | Handle::W => egui::CursorIcon::ResizeHorizontal,
            Handle::NE | Handle::SW => egui::CursorIcon::ResizeNeSw,
            Handle::NW | Handle::SE => egui::CursorIcon::ResizeNwSe,
        }
    }
}

enum Drag {
    Move {
        start: (f64, f64),
        origin: Vec<(usize, [f64; 4])>,
        moved: bool,
    },
    Resize {
        idx: usize,
        handle: Handle,
        orig: [f64; 4],
    },
    Marquee {
        start: Pos2,
        additive: bool,
    },
    Pan {
        last: Pos2,
    },
    Place {
        tool: Tool,
        start: (f64, f64),
    },
}

#[derive(Default)]
pub struct CanvasState {
    drag: Option<Drag>,
    guides_x: Vec<f64>,
    guides_y: Vec<f64>,
    pub cursor_inch: Option<(f64, f64)>,
    /// ダブルクリックで文字欄を編集したい合図（プロパティ欄が拾う）
    pub focus_text: bool,
    pub status: Option<String>,
}

pub enum CanvasAction {
    None,
    ContextDuplicate,
    ContextDelete,
    ContextFront,
    ContextBack,
}

struct Xf {
    origin: Pos2,
    ppi: f32,
}

impl Xf {
    fn pt(&self, x: f64, y: f64) -> Pos2 {
        self.origin + vec2(x as f32 * self.ppi, y as f32 * self.ppi)
    }
    fn rect(&self, r: [f64; 4]) -> Rect {
        Rect::from_two_pos(self.pt(r[0], r[1]), self.pt(r[2], r[3]))
    }
    fn inch(&self, p: Pos2) -> (f64, f64) {
        (
            ((p.x - self.origin.x) / self.ppi) as f64,
            ((p.y - self.origin.y) / self.ppi) as f64,
        )
    }
}

pub fn color(c: u32) -> Color32 {
    let [r, g, b] = tpl::colorref_rgb(c);
    Color32::from_rgb(r, g, b)
}

pub fn font_family(face: &str) -> FontFamily {
    let key = match face {
        f if f.contains("明朝") && (f.contains('Ｐ') || f.contains('P')) => "pmincho",
        f if f.contains("明朝") => "mincho",
        f if f.contains("ゴシック") && !(f.contains('Ｐ') || f.contains('P')) => "gothic",
        _ => "pgothic",
    };
    FontFamily::Name(key.into())
}

/// 項目値の見本文字
fn sample_value(field: &str) -> &'static str {
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

pub fn show(
    ui: &mut egui::Ui,
    doc: &mut Document,
    view: &mut View,
    st: &mut CanvasState,
    tool: &mut Tool,
) -> CanvasAction {
    let frame = card_frame(&doc.template);
    let avail = ui.available_rect_before_wrap();
    let (resp, painter) = ui.allocate_painter(avail.size(), Sense::click_and_drag());
    let area = resp.rect;
    painter.rect_filled(area, 0.0, Color32::from_rgb(0xdf, 0xe3, 0xe8));

    let work = Rect::from_min_max(area.min + vec2(RULER, RULER), area.max);
    if view.fit_pending && work.width() > 50.0 {
        let zx = (work.width() - 80.0) / (frame.width as f32 * PX_PER_INCH);
        let zy = (work.height() - 80.0) / (frame.height as f32 * PX_PER_INCH);
        view.zoom = zx.min(zy).clamp(0.2, 8.0);
        let ppi = PX_PER_INCH * view.zoom;
        view.pan = vec2(
            (work.width() - frame.width as f32 * ppi) / 2.0,
            (work.height() - frame.height as f32 * ppi) / 2.0,
        );
        view.fit_pending = false;
    }

    // ----- 拡大縮小とスクロール -----
    let hover = resp.hover_pos();
    if let Some(hp) = hover {
        let (zoom_delta, scroll) = ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta));
        if zoom_delta != 1.0 {
            let old = view.zoom;
            view.zoom = (view.zoom * zoom_delta).clamp(0.2, 8.0);
            let anchor = hp - work.min - view.pan;
            view.pan += anchor - anchor * (view.zoom / old);
        } else if scroll != Vec2::ZERO {
            view.pan += scroll;
        }
    }
    let xf = Xf {
        origin: work.min + view.pan,
        ppi: PX_PER_INCH * view.zoom,
    };
    st.cursor_inch = hover.filter(|p| work.contains(*p)).map(|p| xf.inch(p));

    let clip = painter.with_clip_rect(work);
    draw_card(&clip, &xf, &frame, view, doc.template.card_color);

    // ----- 物の描画 -----
    let mut picture_no = 0;
    for (i, o) in doc.template.objects.iter().enumerate() {
        if o.kind == tpl::KIND_PICTURE {
            picture_no += 1;
        }
        draw_object(&clip, &xf, o, picture_no, view.sample_values);
        let _ = i;
    }

    // ----- 入力 -----
    let ppi = xf.ppi as f64;
    let snap_tol = SNAP_PX as f64 / ppi;
    let grid = view.grid_mm as f64 / MM_PER_INCH;
    let shift = ui.input(|i| i.modifiers.shift);
    let alt = ui.input(|i| i.modifiers.alt);
    let space = ui.input(|i| i.key_down(egui::Key::Space));
    let single_sel: Option<usize> = match doc.selected_editable().as_slice() {
        [i] => Some(*i),
        _ => None,
    };

    // 掴む場所の判定
    let pointer = resp.interact_pointer_pos().or(hover);
    let handle_under = |doc: &Document, p: Pos2| -> Option<(usize, Handle)> {
        let i = single_sel?;
        let r = xf.rect(doc.template.objects[i].normalized());
        Handle::ALL
            .into_iter()
            .find(|h| Rect::from_center_size(h.pos(r), Vec2::splat(HANDLE + 6.0)).contains(p))
            .map(|h| (i, h))
    };
    let hit = |doc: &Document, p: Pos2| -> Option<usize> {
        let (x, y) = xf.inch(p);
        let tol = 3.0 / ppi;
        doc.template
            .objects
            .iter()
            .enumerate()
            .rev()
            .find_map(|(i, o)| {
                let n = o.normalized();
                (x >= n[0] - tol && x <= n[2] + tol && y >= n[1] - tol && y <= n[3] + tol)
                    .then_some(i)
            })
    };

    if let Some(p) = hover
        && st.drag.is_none()
    {
        if *tool != Tool::Select {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        } else if let Some((_, h)) = handle_under(doc, p) {
            ui.ctx().set_cursor_icon(h.cursor());
        } else if let Some(i) = hit(doc, p) {
            ui.ctx().set_cursor_icon(if doc.editable(i) {
                egui::CursorIcon::Move
            } else {
                egui::CursorIcon::NotAllowed
            });
            let r = xf.rect(doc.template.objects[i].normalized());
            clip.rect_stroke(
                r.expand(1.5),
                2.0,
                Stroke::new(1.0, ACCENT.gamma_multiply(0.5)),
                StrokeKind::Outside,
            );
        }
    }

    if resp.drag_started_by(egui::PointerButton::Middle) || (resp.drag_started() && space) {
        if let Some(p) = pointer {
            st.drag = Some(Drag::Pan { last: p });
        }
    } else if resp.drag_started_by(egui::PointerButton::Primary)
        && let Some(p) = resp.interact_pointer_pos()
    {
        let p0 = p - resp.drag_delta();
        if *tool != Tool::Select {
            let s = snap_point(xf.inch(p0), grid, view.snap_grid);
            st.drag = Some(Drag::Place {
                tool: *tool,
                start: s,
            });
        } else if let Some((idx, handle)) = handle_under(doc, p0) {
            doc.checkpoint(None);
            st.drag = Some(Drag::Resize {
                idx,
                handle,
                orig: doc.template.objects[idx].normalized(),
            });
        } else if let Some(i) = hit(doc, p0) {
            if !doc.selection.contains(&i) {
                if !shift {
                    doc.selection.clear();
                }
                doc.selection.insert(i);
            }
            let origin: Vec<(usize, [f64; 4])> = doc
                .selected_editable()
                .into_iter()
                .map(|k| (k, doc.template.objects[k].rect))
                .collect();
            if origin.is_empty() {
                st.status = Some(
                    "この部品は本体の形式のまま保持しています（移動・変更はできません）".into(),
                );
            } else {
                doc.checkpoint(None);
                if alt {
                    // Alt を押しながらドラッグで複製
                    let copies: Vec<Object> = origin
                        .iter()
                        .map(|(k, _)| doc.template.objects[*k].clone())
                        .collect();
                    let start = doc.template.objects.len();
                    doc.template.objects.extend(copies);
                    doc.selection = (start..doc.template.objects.len()).collect();
                    let origin = doc
                        .selection
                        .iter()
                        .map(|&k| (k, doc.template.objects[k].rect))
                        .collect();
                    st.drag = Some(Drag::Move {
                        start: xf.inch(p0),
                        origin,
                        moved: false,
                    });
                } else {
                    st.drag = Some(Drag::Move {
                        start: xf.inch(p0),
                        origin,
                        moved: false,
                    });
                }
            }
        } else {
            if !shift {
                doc.selection.clear();
            }
            st.drag = Some(Drag::Marquee {
                start: p0,
                additive: shift,
            });
        }
    }

    st.guides_x.clear();
    st.guides_y.clear();
    if let Some(p) = pointer {
        match &mut st.drag {
            Some(Drag::Pan { last }) => {
                view.pan += p - *last;
                *last = p;
            }
            Some(Drag::Move {
                start,
                origin,
                moved,
            }) => {
                let (x, y) = xf.inch(p);
                let (mut dx, mut dy) = (x - start.0, y - start.1);
                if shift {
                    // Shift で縦横どちらかに固定
                    if dx.abs() > dy.abs() {
                        dy = 0.0;
                    } else {
                        dx = 0.0;
                    }
                }
                let moving: Vec<usize> = origin.iter().map(|(k, _)| *k).collect();
                let b = bounds_of(origin.iter().map(|(_, r)| norm(*r)));
                let moved_b = [b[0] + dx, b[1] + dy, b[2] + dx, b[3] + dy];
                let (cx, cy) = candidates(doc, &frame, &moving, view.snap_objects);
                let (sx, gx) = best_snap(
                    &[moved_b[0], (moved_b[0] + moved_b[2]) / 2.0, moved_b[2]],
                    &cx,
                    snap_tol,
                );
                let (sy, gy) = best_snap(
                    &[moved_b[1], (moved_b[1] + moved_b[3]) / 2.0, moved_b[3]],
                    &cy,
                    snap_tol,
                );
                if let Some(g) = gx {
                    dx += sx;
                    st.guides_x.push(g);
                } else if view.snap_grid {
                    dx += snap(moved_b[0], grid) - moved_b[0];
                }
                if let Some(g) = gy {
                    dy += sy;
                    st.guides_y.push(g);
                } else if view.snap_grid {
                    dy += snap(moved_b[1], grid) - moved_b[1];
                }
                if dx.abs() > 1e-9 || dy.abs() > 1e-9 {
                    *moved = true;
                }
                for (k, r) in origin.iter() {
                    doc.template.objects[*k].rect = [r[0] + dx, r[1] + dy, r[2] + dx, r[3] + dy];
                }
                st.status = Some(format!(
                    "移動 {:+.1} mm, {:+.1} mm",
                    dx * MM_PER_INCH,
                    dy * MM_PER_INCH
                ));
            }
            Some(Drag::Resize { idx, handle, orig }) => {
                let (mut x, mut y) = xf.inch(p);
                let (ml, mt, mr, mb) = handle.moves();
                let (cx, cy) = candidates(doc, &frame, &[*idx], view.snap_objects);
                let (sx, gx) = best_snap(&[x], &cx, snap_tol);
                let (sy, gy) = best_snap(&[y], &cy, snap_tol);
                if let Some(g) = gx {
                    x += sx;
                    st.guides_x.push(g);
                } else if view.snap_grid {
                    x = snap(x, grid);
                }
                if let Some(g) = gy {
                    y += sy;
                    st.guides_y.push(g);
                } else if view.snap_grid {
                    y = snap(y, grid);
                }
                let min = 1.0 / MM_PER_INCH;
                let mut r = *orig;
                if ml {
                    r[0] = x.min(r[2] - min);
                }
                if mr {
                    r[2] = x.max(r[0] + min);
                }
                if mt {
                    r[1] = y.min(r[3] - min);
                }
                if mb {
                    r[3] = y.max(r[1] + min);
                }
                doc.template.objects[*idx].rect = r;
                st.status = Some(format!(
                    "幅 {:.1} mm × 高さ {:.1} mm",
                    (r[2] - r[0]) * MM_PER_INCH,
                    (r[3] - r[1]) * MM_PER_INCH
                ));
            }
            Some(Drag::Marquee { start, .. }) => {
                let r = Rect::from_two_pos(*start, p);
                clip.rect(
                    r,
                    0.0,
                    ACCENT.gamma_multiply(0.08),
                    Stroke::new(1.0, ACCENT),
                    StrokeKind::Inside,
                );
            }
            Some(Drag::Place { tool: t, start }) => {
                let e = snap_point(xf.inch(p), grid, view.snap_grid);
                let r = xf.rect([start.0, start.1, e.0, e.1]);
                clip.rect(
                    r,
                    0.0,
                    ACCENT.gamma_multiply(0.08),
                    Stroke::new(1.5, ACCENT),
                    StrokeKind::Inside,
                );
                clip.text(
                    r.left_top() + vec2(4.0, -4.0),
                    Align2::LEFT_BOTTOM,
                    format!(
                        "{}  {:.1}×{:.1} mm",
                        t.label(),
                        (e.0 - start.0).abs() * MM_PER_INCH,
                        (e.1 - start.1).abs() * MM_PER_INCH
                    ),
                    FontId::proportional(12.0),
                    ACCENT,
                );
            }
            None => {}
        }
    }

    if resp.drag_stopped() {
        match st.drag.take() {
            Some(Drag::Marquee { start, additive }) => {
                if let Some(p) = pointer {
                    let sel = Rect::from_two_pos(start, p);
                    let (a, b) = (xf.inch(sel.min), xf.inch(sel.max));
                    for (i, o) in doc.template.objects.iter().enumerate() {
                        let n = o.normalized();
                        if n[0] >= a.0 && n[2] <= b.0 && n[1] >= a.1 && n[3] <= b.1 {
                            doc.selection.insert(i);
                        }
                    }
                    let _ = additive;
                }
            }
            Some(Drag::Place { tool: t, start }) => {
                if let Some(p) = pointer {
                    let e = snap_point(xf.inch(p), grid, view.snap_grid);
                    place(doc, t, start, e);
                    *tool = Tool::Select;
                }
            }
            Some(Drag::Move { moved: false, .. }) => doc.end_coalesce(),
            _ => doc.end_coalesce(),
        }
        st.status = None;
    }

    if resp.clicked()
        && let Some(p) = resp.interact_pointer_pos()
    {
        if *tool != Tool::Select {
            let s = snap_point(xf.inch(p), grid, view.snap_grid);
            let (w, h) = default_size(*tool);
            place(doc, *tool, s, (s.0 + w, s.1 + h));
            *tool = Tool::Select;
        } else if let Some(i) = hit(doc, p) {
            if shift {
                if !doc.selection.remove(&i) {
                    doc.selection.insert(i);
                }
            } else {
                doc.selection = [i].into();
            }
        } else if !shift {
            doc.selection.clear();
        }
    }
    if resp.double_clicked()
        && let Some(p) = resp.interact_pointer_pos()
        && let Some(i) = hit(doc, p)
        && matches!(doc.template.objects[i].body, Body::Caption(_))
    {
        doc.selection = [i].into();
        st.focus_text = true;
    }

    // ----- 選択の表示 -----
    for &i in &doc.selection {
        let Some(o) = doc.template.objects.get(i) else {
            continue;
        };
        let r = xf.rect(o.normalized());
        let c = if o.is_passthrough() {
            Color32::from_rgb(0x99, 0x99, 0x99)
        } else {
            ACCENT
        };
        clip.rect_stroke(r.expand(1.0), 1.0, Stroke::new(1.5, c), StrokeKind::Outside);
    }
    if let Some(i) = single_sel {
        let r = xf.rect(doc.template.objects[i].normalized());
        for h in Handle::ALL {
            let hr = Rect::from_center_size(h.pos(r), Vec2::splat(HANDLE));
            clip.rect(
                hr,
                1.5,
                Color32::WHITE,
                Stroke::new(1.3, ACCENT),
                StrokeKind::Middle,
            );
        }
        // 大きさの吹き出し
        let o = &doc.template.objects[i];
        let n = o.normalized();
        let label = format!(
            "{:.1} × {:.1} mm",
            (n[2] - n[0]) * MM_PER_INCH,
            (n[3] - n[1]) * MM_PER_INCH
        );
        let tag = clip.text(
            r.center_bottom() + vec2(0.0, 10.0),
            Align2::CENTER_TOP,
            &label,
            FontId::proportional(11.0),
            Color32::WHITE,
        );
        clip.rect_filled(tag.expand2(vec2(5.0, 2.0)), 4.0, ACCENT);
        clip.text(
            r.center_bottom() + vec2(0.0, 10.0),
            Align2::CENTER_TOP,
            &label,
            FontId::proportional(11.0),
            Color32::WHITE,
        );
    }
    for &g in &st.guides_x {
        let x = xf.pt(g, 0.0).x;
        clip.line_segment(
            [pos2(x, work.top()), pos2(x, work.bottom())],
            Stroke::new(1.0, GUIDE),
        );
    }
    for &g in &st.guides_y {
        let y = xf.pt(0.0, g).y;
        clip.line_segment(
            [pos2(work.left(), y), pos2(work.right(), y)],
            Stroke::new(1.0, GUIDE),
        );
    }

    draw_rulers(&painter, area, &xf, st.cursor_inch);

    // ----- 右クリックの操作 -----
    let mut action = CanvasAction::None;
    if resp.secondary_clicked()
        && let Some(p) = resp.interact_pointer_pos()
        && let Some(i) = hit(doc, p)
        && !doc.selection.contains(&i)
    {
        doc.selection = [i].into();
    }
    resp.context_menu(|ui| {
        let any = !doc.selected_editable().is_empty();
        if ui
            .add_enabled(any, egui::Button::new("複製").shortcut_text("Ctrl+D"))
            .clicked()
        {
            action = CanvasAction::ContextDuplicate;
            ui.close();
        }
        if ui
            .add_enabled(any, egui::Button::new("削除").shortcut_text("Delete"))
            .clicked()
        {
            action = CanvasAction::ContextDelete;
            ui.close();
        }
        ui.separator();
        if ui.add_enabled(any, egui::Button::new("最前面へ")).clicked() {
            action = CanvasAction::ContextFront;
            ui.close();
        }
        if ui.add_enabled(any, egui::Button::new("最背面へ")).clicked() {
            action = CanvasAction::ContextBack;
            ui.close();
        }
    });
    action
}

fn default_size(tool: Tool) -> (f64, f64) {
    let mm = |v: f64| v / MM_PER_INCH;
    match tool {
        Tool::Picture => (mm(80.0), mm(60.0)),
        Tool::Row => (mm(70.0), mm(5.0)),
        _ => (mm(25.0), mm(5.0)),
    }
}

fn place(doc: &mut Document, tool: Tool, a: (f64, f64), b: (f64, f64)) {
    let mut r = [a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)];
    let (dw, dh) = default_size(tool);
    // 小さすぎる（クリックに近い）ときは既定の大きさ
    if r[2] - r[0] < 2.0 / MM_PER_INCH {
        r[2] = r[0] + dw;
    }
    if r[3] - r[1] < 2.0 / MM_PER_INCH {
        r[3] = r[1] + dh;
    }
    match tool {
        Tool::Picture => {
            doc.add_picture(r);
        }
        Tool::Label => {
            doc.add_caption("項目名", "", false, r);
        }
        Tool::Value => {
            doc.add_caption("", "写真タイトル", true, r);
        }
        Tool::Row => {
            let w = r[2] - r[0];
            let (a, b) = doc.add_field_row(
                "写真タイトル",
                (r[0], r[1]),
                w * 0.32,
                w * 0.68,
                r[3] - r[1],
            );
            doc.selection = [a, b].into();
        }
        Tool::Select => {}
    }
}

fn norm(r: [f64; 4]) -> [f64; 4] {
    [
        r[0].min(r[2]),
        r[1].min(r[3]),
        r[0].max(r[2]),
        r[1].max(r[3]),
    ]
}

fn bounds_of(it: impl Iterator<Item = [f64; 4]>) -> [f64; 4] {
    it.fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |a, b| {
        [
            a[0].min(b[0]),
            a[1].min(b[1]),
            a[2].max(b[2]),
            a[3].max(b[3]),
        ]
    })
}

fn snap(v: f64, grid: f64) -> f64 {
    if grid <= 0.0 {
        v
    } else {
        (v / grid).round() * grid
    }
}

fn snap_point(p: (f64, f64), grid: f64, on: bool) -> (f64, f64) {
    if on {
        (snap(p.0, grid), snap(p.1, grid))
    } else {
        p
    }
}

/// 吸着先: 他の物の左・中央・右（上・中央・下）と、カードの端・中央
fn candidates(
    doc: &Document,
    frame: &CardFrame,
    skip: &[usize],
    objects: bool,
) -> (Vec<f64>, Vec<f64>) {
    let mut xs = vec![0.0, frame.width / 2.0, frame.width];
    let mut ys = vec![0.0, frame.height / 2.0, frame.height];
    if let Some(u) = frame.used {
        xs.extend([u[0], u[2]]);
        ys.extend([u[1], u[3]]);
    }
    if objects {
        for (i, o) in doc.template.objects.iter().enumerate() {
            if skip.contains(&i) {
                continue;
            }
            let n = o.normalized();
            xs.extend([n[0], (n[0] + n[2]) / 2.0, n[2]]);
            ys.extend([n[1], (n[1] + n[3]) / 2.0, n[3]]);
        }
    }
    (xs, ys)
}

/// 動かす側の基準線（左・中央・右など）のうち、吸着先に最も近いものを探す
fn best_snap(edges: &[f64], targets: &[f64], tol: f64) -> (f64, Option<f64>) {
    let mut best: Option<(f64, f64)> = None;
    for &e in edges {
        for &t in targets {
            let d = t - e;
            if d.abs() <= tol && best.is_none_or(|(bd, _)| d.abs() < bd.abs()) {
                best = Some((d, t));
            }
        }
    }
    match best {
        Some((d, t)) => (d, Some(t)),
        None => (0.0, None),
    }
}

// ---------------------------------------------------------------------------
// 描画

fn draw_card(p: &egui::Painter, xf: &Xf, frame: &CardFrame, view: &View, card_color: u32) {
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

fn draw_object(p: &egui::Painter, xf: &Xf, o: &Object, picture_no: usize, samples: bool) {
    let r = xf.rect(o.normalized());
    match &o.body {
        Body::Picture(_) => {
            p.rect(
                r,
                0.0,
                PHOTO_FILL,
                Stroke::new(1.0, PHOTO_EDGE),
                StrokeKind::Inside,
            );
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
            let (text, placeholder) = if c.is_value != 0 {
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

fn draw_caption_text(
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
    let mut job = LayoutJob::default();
    let tf = egui::TextFormat {
        font_id: FontId::new(px, family),
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
    let galley = clip.layout_job(job);
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
    clip.galley(pos, galley.clone(), col);
    if bold {
        clip.galley(pos + vec2((px / 28.0).max(0.6), 0.0), galley, col);
    }
}

fn draw_passthrough(p: &egui::Painter, xf: &Xf, o: &Object, r: Rect) {
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

fn draw_rulers(p: &egui::Painter, area: Rect, xf: &Xf, cursor: Option<(f64, f64)>) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::card_frame;
    use crate::test_support::Harness;
    use crate::tpl::{KIND_PICTURE, Template};
    use egui::Modifiers;

    #[test]
    fn snapping_picks_nearest_target() {
        let cases = [
            (vec![1.0, 2.0], vec![1.05, 3.0], 0.1, (0.05, Some(1.05))),
            (vec![1.0], vec![2.0], 0.1, (0.0, None)),
            (vec![0.0, 5.0], vec![4.98, 0.09], 0.1, (-0.02, Some(4.98))),
        ];
        for (edges, targets, tol, (d, g)) in cases {
            let (bd, bg) = best_snap(&edges, &targets, tol);
            assert!((bd - d).abs() < 1e-9, "{edges:?}");
            assert_eq!(bg, g);
        }
    }

    #[test]
    fn font_family_from_face_name() {
        let cases = [
            ("ＭＳ Ｐゴシック", "pgothic"),
            ("ＭＳ ゴシック", "gothic"),
            ("ＭＳ Ｐ明朝", "pmincho"),
            ("ＭＳ 明朝", "mincho"),
            ("Arial", "pgothic"),
        ];
        for (face, key) in cases {
            assert_eq!(font_family(face), FontFamily::Name(key.into()), "{face}");
        }
    }

    // ----- 純粋な計算 -----

    #[test]
    fn rect_helpers() {
        assert_eq!(norm([2.0, 3.0, 1.0, 0.5]), [1.0, 0.5, 2.0, 3.0]);
        assert_eq!(norm([0.0, 0.0, 1.0, 1.0]), [0.0, 0.0, 1.0, 1.0]);
        let b = bounds_of([[1.0, 1.0, 2.0, 2.0], [0.5, 1.5, 3.0, 1.8]].into_iter());
        assert_eq!(b, [0.5, 1.0, 3.0, 2.0]);
    }

    #[test]
    fn snap_to_grid() {
        let cases = [
            (1.04, 0.1, 1.0),
            (1.06, 0.1, 1.1),
            (1.234, 0.0, 1.234),
            (1.234, -1.0, 1.234),
        ];
        for (v, grid, expected) in cases {
            assert!((snap(v, grid) - expected).abs() < 1e-9, "{v} {grid}");
        }
        assert_eq!(snap_point((1.04, 2.06), 0.1, false), (1.04, 2.06));
        let (x, y) = snap_point((1.04, 2.06), 0.1, true);
        assert!((x - 1.0).abs() < 1e-9 && (y - 2.1).abs() < 1e-9);
    }

    #[test]
    fn snap_candidates_include_card_and_neighbors() {
        let t = Template {
            objects: vec![cap([1.0, 1.0, 2.0, 1.5]), cap([3.0, 2.0, 4.0, 3.0])],
            ..Template::default()
        };
        let doc = Document::new(t, "t", None);
        let frame = card_frame(&doc.template);
        let (xs, ys) = candidates(&doc, &frame, &[0], true);
        assert!(xs.contains(&0.0) && xs.contains(&frame.width));
        assert!(xs.contains(&3.0) && xs.contains(&3.5) && xs.contains(&4.0));
        assert!(!xs.contains(&1.5), "動かす物自身は吸着先にしない");
        assert!(ys.contains(&2.0) && ys.contains(&3.0));
        let (xs, _) = candidates(&doc, &frame, &[], false);
        assert!(!xs.contains(&3.0), "物への吸着を切ると物の端は出ない");
        let used = frame.used.expect("1x3 は実測あり");
        assert!(xs.contains(&used[0]) && xs.contains(&used[2]));
    }

    #[test]
    fn default_sizes_follow_tool() {
        let cases = [
            (Tool::Picture, 80.0, 60.0),
            (Tool::Row, 70.0, 5.0),
            (Tool::Label, 25.0, 5.0),
            (Tool::Value, 25.0, 5.0),
        ];
        for (tool, w, h) in cases {
            let (dw, dh) = default_size(tool);
            assert!((dw * MM_PER_INCH - w).abs() < 1e-9, "{tool:?}");
            assert!((dh * MM_PER_INCH - h).abs() < 1e-9, "{tool:?}");
        }
    }

    #[test]
    fn tool_labels_and_hints_are_distinct() {
        let all = [
            Tool::Select,
            Tool::Picture,
            Tool::Label,
            Tool::Value,
            Tool::Row,
        ];
        let labels: std::collections::BTreeSet<_> = all.iter().map(|t| t.label()).collect();
        let hints: std::collections::BTreeSet<_> = all.iter().map(|t| t.hint()).collect();
        assert_eq!(labels.len(), all.len());
        assert_eq!(hints.len(), all.len());
    }

    #[test]
    fn handles_cover_all_edges() {
        let r = Rect::from_min_max(pos2(10.0, 20.0), pos2(30.0, 60.0));
        let cases = [
            (Handle::NW, pos2(10.0, 20.0), (true, true, false, false)),
            (Handle::N, pos2(20.0, 20.0), (false, true, false, false)),
            (Handle::NE, pos2(30.0, 20.0), (false, true, true, false)),
            (Handle::E, pos2(30.0, 40.0), (false, false, true, false)),
            (Handle::SE, pos2(30.0, 60.0), (false, false, true, true)),
            (Handle::S, pos2(20.0, 60.0), (false, false, false, true)),
            (Handle::SW, pos2(10.0, 60.0), (true, false, false, true)),
            (Handle::W, pos2(10.0, 40.0), (true, false, false, false)),
        ];
        assert_eq!(Handle::ALL.len(), cases.len());
        for (h, pos, moves) in cases {
            assert_eq!(h.pos(r), pos, "{h:?}");
            assert_eq!(h.moves(), moves, "{h:?}");
            let _ = h.cursor();
        }
    }

    #[test]
    fn transform_roundtrips_between_inch_and_screen() {
        let xf = Xf {
            origin: pos2(100.0, 50.0),
            ppi: 200.0,
        };
        assert_eq!(xf.pt(1.0, 0.5), pos2(300.0, 150.0));
        assert_eq!(xf.inch(pos2(300.0, 150.0)), (1.0, 0.5));
        let r = xf.rect([1.0, 0.5, 0.0, 0.0]);
        assert_eq!(r.min, pos2(100.0, 50.0));
        assert_eq!(r.max, pos2(300.0, 150.0));
    }

    #[test]
    fn colorref_converts_to_color32() {
        assert_eq!(color(0x00ff_0000), Color32::from_rgb(0, 0, 255));
        assert_eq!(color(0x0000_00ff), Color32::from_rgb(255, 0, 0));
    }

    #[test]
    fn sample_values_cover_known_fields() {
        for f in crate::doc::KNOWN_FIELDS {
            let s = sample_value(f);
            assert!(!s.is_empty(), "{f}");
        }
        assert_eq!(sample_value("未知の項目"), "（データ）");
        assert_eq!(sample_value("写真タイトル"), "床掘状況");
    }

    #[test]
    fn place_creates_objects_per_tool() {
        let cases = [
            (Tool::Picture, 1usize, KIND_PICTURE),
            (Tool::Label, 1, tpl::KIND_CAPTION),
            (Tool::Value, 1, tpl::KIND_CAPTION),
            (Tool::Row, 2, tpl::KIND_CAPTION),
            (Tool::Select, 0, 0),
        ];
        for (tool, count, kind) in cases {
            let mut doc = Document::new(Template::default(), "t", None);
            place(&mut doc, tool, (1.0, 1.0), (3.0, 2.0));
            assert_eq!(doc.template.objects.len(), count, "{tool:?}");
            if let Some(o) = doc.template.objects.first() {
                assert_eq!(o.kind, kind, "{tool:?}");
                if tool != Tool::Row {
                    assert_eq!(o.rect, [1.0, 1.0, 3.0, 2.0], "{tool:?}");
                }
            }
        }
    }

    #[test]
    fn place_uses_default_size_for_tiny_drags() {
        let mut doc = Document::new(Template::default(), "t", None);
        place(&mut doc, Tool::Picture, (1.0, 1.0), (1.0, 1.0));
        let (dw, dh) = default_size(Tool::Picture);
        let r = doc.template.objects[0].rect;
        assert!((r[2] - r[0] - dw).abs() < 1e-9 && (r[3] - r[1] - dh).abs() < 1e-9);
        // 逆向きのドラッグでも正規化される
        let mut doc = Document::new(Template::default(), "t", None);
        place(&mut doc, Tool::Value, (3.0, 2.0), (1.0, 1.0));
        assert_eq!(doc.template.objects[0].rect, [1.0, 1.0, 3.0, 2.0]);
    }

    #[test]
    fn place_row_selects_both_cells() {
        let mut doc = Document::new(Template::default(), "t", None);
        place(&mut doc, Tool::Row, (1.0, 1.0), (4.0, 1.5));
        assert_eq!(doc.selection, [0, 1].into());
        let (a, b) = (doc.template.objects[0].rect, doc.template.objects[1].rect);
        assert!((a[2] - a[0] - 0.96).abs() < 1e-9, "名前側は幅の 32%");
        assert!((b[0] - a[2]).abs() < 1e-9, "値側は名前側の右に隣接");
    }

    // ----- 画面なしで show() を動かす -----

    fn cap(rect: [f64; 4]) -> Object {
        Object::new(
            Body::Caption(tpl::CaptionBody::new("a", "", false, 12)),
            rect,
        )
    }

    fn pic(rect: [f64; 4]) -> Object {
        Object::new(Body::Picture(tpl::PictureBody::default()), rect)
    }

    fn locked(rect: [f64; 4]) -> Object {
        Object {
            tag_version: 3,
            kind: tpl::KIND_RECT,
            link: 0,
            rect,
            body: Body::Passthrough(vec![0; 81]),
            trailer: Some(0),
        }
    }

    struct Rig {
        h: Harness,
        doc: Document,
        view: View,
        st: CanvasState,
        tool: Tool,
    }

    impl Rig {
        /// 吸着を切って、座標どおりに動くようにした状態で始める
        fn new(objects: Vec<Object>) -> Self {
            let t = Template {
                objects,
                ..Template::default()
            };
            let view = View {
                snap_grid: false,
                snap_objects: false,
                ..View::default()
            };
            let mut rig = Self {
                h: Harness::new(),
                doc: Document::new(t, "t", None),
                view,
                st: CanvasState::default(),
                tool: Tool::Select,
            };
            rig.frame();
            rig
        }

        fn frame(&mut self) -> CanvasAction {
            let Self {
                h,
                doc,
                view,
                st,
                tool,
            } = self;
            let mut action = CanvasAction::None;
            h.frame(|ui| action = show(ui, doc, view, st, tool));
            action
        }

        /// インチ座標 → 画面座標
        fn at(&self, x: f64, y: f64) -> Pos2 {
            let ppi = PX_PER_INCH * self.view.zoom;
            pos2(RULER, RULER) + self.view.pan + vec2(x as f32 * ppi, y as f32 * ppi)
        }

        fn click(&mut self, p: Pos2) {
            self.h.pointer_to(p);
            self.frame();
            self.h.press(p);
            self.frame();
            self.h.release(p);
            self.frame();
        }

        /// 押す → 動かす → 離す を 1 フレームずつ
        fn drag(&mut self, a: Pos2, b: Pos2) {
            self.h.pointer_to(a);
            self.frame();
            self.h.press(a);
            self.frame();
            self.h.pointer_to(b);
            self.frame();
            self.h.release(b);
            self.frame();
        }

        fn rect_of(&self, i: usize) -> [f64; 4] {
            self.doc.template.objects[i].rect
        }
    }

    fn assert_rect(actual: [f64; 4], expected: [f64; 4]) {
        for k in 0..4 {
            assert!(
                (actual[k] - expected[k]).abs() < 2e-3,
                "{actual:?} != {expected:?}"
            );
        }
    }

    fn two_captions_and_picture() -> Vec<Object> {
        vec![
            cap([1.0, 1.0, 2.0, 1.5]),
            cap([3.0, 1.0, 4.0, 1.5]),
            pic([5.0, 1.0, 6.0, 2.0]),
        ]
    }

    #[test]
    fn first_frame_fits_card_into_view() {
        let rig = Rig::new(vec![]);
        assert!(!rig.view.fit_pending);
        assert!(rig.view.zoom > 0.2 && rig.view.zoom <= 8.0);
        let frame = card_frame(&rig.doc.template);
        let far = rig.at(frame.width, frame.height);
        assert!(far.x <= rig.h.screen.right() && far.y <= rig.h.screen.bottom());
    }

    #[test]
    fn click_selection() {
        let a = (1.5, 1.25);
        let b = (3.5, 1.25);
        let empty = (1.5, 3.0);
        // (クリックの並び（位置, Shift）, 期待する選択)
        type Clicks = Vec<((f64, f64), bool)>;
        let cases: Vec<(Clicks, Vec<usize>)> = vec![
            (vec![(a, false)], vec![0]),
            (vec![(a, false), (b, false)], vec![1]),
            (vec![(a, false), (b, true)], vec![0, 1]),
            (vec![(a, false), (a, true)], vec![]),
            (vec![(a, false), (empty, false)], vec![]),
            (vec![(a, false), (empty, true)], vec![0]),
        ];
        for (clicks, expected) in cases {
            let mut rig = Rig::new(two_captions_and_picture());
            for ((x, y), shift) in &clicks {
                rig.h.hold(if *shift {
                    Modifiers::SHIFT
                } else {
                    Modifiers::NONE
                });
                let p = rig.at(*x, *y);
                rig.click(p);
                rig.h.wait(1.0);
            }
            let sel: Vec<usize> = rig.doc.selection.iter().copied().collect();
            assert_eq!(sel, expected, "{clicks:?}");
        }
    }

    #[test]
    fn hover_over_object_and_handle_does_not_change_document() {
        let mut rig = Rig::new(two_captions_and_picture());
        let before = rig.doc.template.clone();
        let p = rig.at(1.5, 1.25);
        rig.h.pointer_to(p);
        rig.frame();
        rig.click(p);
        let corner = rig.at(2.0, 1.5);
        rig.h.pointer_to(corner);
        rig.frame();
        assert_eq!(rig.doc.template, before);
        assert!(!rig.doc.dirty);
    }

    #[test]
    fn drag_moves_object_in_one_undo_step() {
        let mut rig = Rig::new(two_captions_and_picture());
        let before = rig.doc.template.clone();
        let (a, b) = (rig.at(1.5, 1.25), rig.at(2.0, 1.75));
        rig.drag(a, b);
        assert_rect(rig.rect_of(0), [1.5, 1.5, 2.5, 2.0]);
        assert_rect(rig.rect_of(1), [3.0, 1.0, 4.0, 1.5]);
        assert!(rig.doc.dirty);
        assert!(rig.st.status.is_none(), "ドラッグが終われば表示を消す");
        rig.doc.undo();
        assert_eq!(rig.doc.template, before);
    }

    #[test]
    fn drag_moves_all_selected_objects() {
        let mut rig = Rig::new(two_captions_and_picture());
        rig.doc.selection = [0, 1].into();
        let (a, b) = (rig.at(1.5, 1.25), rig.at(1.5, 2.25));
        rig.drag(a, b);
        assert_rect(rig.rect_of(0), [1.0, 2.0, 2.0, 2.5]);
        assert_rect(rig.rect_of(1), [3.0, 2.0, 4.0, 2.5]);
        assert_rect(rig.rect_of(2), [5.0, 1.0, 6.0, 2.0]);
    }

    #[test]
    fn shift_drag_locks_to_dominant_axis() {
        let mut rig = Rig::new(two_captions_and_picture());
        rig.h.hold(Modifiers::SHIFT);
        let (a, b) = (rig.at(1.5, 1.25), rig.at(2.0, 1.5));
        rig.drag(a, b);
        assert_rect(rig.rect_of(0), [1.5, 1.0, 2.5, 1.5]);
    }

    #[test]
    fn drag_status_shows_moved_distance_while_dragging() {
        let mut rig = Rig::new(two_captions_and_picture());
        let (a, b) = (rig.at(1.5, 1.25), rig.at(2.0, 1.25));
        rig.h.pointer_to(a);
        rig.frame();
        rig.h.press(a);
        rig.frame();
        rig.h.pointer_to(b);
        rig.frame();
        let status = rig.st.status.clone().expect("移動量を出す");
        assert!(status.starts_with("移動 "), "{status}");
        rig.h.release(b);
        rig.frame();
        assert!(rig.st.status.is_none());
    }

    #[test]
    fn drag_snaps_to_neighbor_edges_and_shows_guide() {
        let mut rig = Rig::new(two_captions_and_picture());
        rig.view.snap_objects = true;
        // 右端 2.0 を 2.96 まで動かすと、隣の左端 3.0 に吸い付く
        let (a, b) = (rig.at(1.5, 1.25), rig.at(2.46, 1.25));
        rig.h.pointer_to(a);
        rig.frame();
        rig.h.press(a);
        rig.frame();
        rig.h.pointer_to(b);
        rig.frame();
        assert!(rig.st.guides_x.contains(&3.0), "{:?}", rig.st.guides_x);
        rig.h.release(b);
        rig.frame();
        assert_rect(rig.rect_of(0), [2.0, 1.0, 3.0, 1.5]);
        // 離したフレームではまだ描かれ、次のフレームで消える
        rig.frame();
        assert!(rig.st.guides_x.is_empty(), "{:?}", rig.st.guides_x);
    }

    #[test]
    fn drag_snaps_to_grid_when_enabled() {
        let mut rig = Rig::new(vec![cap([1.0, 1.0, 2.0, 1.5])]);
        rig.view.snap_grid = true;
        rig.view.grid_mm = 5.0;
        let step = 5.0 / MM_PER_INCH;
        let (a, b) = (rig.at(1.5, 1.25), rig.at(1.5 + 0.31, 1.25 + 0.31));
        rig.drag(a, b);
        let r = rig.rect_of(0);
        assert!(((r[0] / step).round() * step - r[0]).abs() < 1e-6, "{r:?}");
        assert!(((r[1] / step).round() * step - r[1]).abs() < 1e-6, "{r:?}");
    }

    #[test]
    fn locked_object_cannot_be_dragged() {
        let mut rig = Rig::new(vec![locked([1.0, 1.0, 2.0, 2.0])]);
        let before = rig.doc.template.clone();
        let (a, b) = (rig.at(1.5, 1.5), rig.at(2.5, 2.5));
        rig.h.pointer_to(a);
        rig.frame();
        rig.h.press(a);
        rig.frame();
        rig.h.pointer_to(b);
        rig.frame();
        assert!(
            rig.st
                .status
                .as_deref()
                .is_some_and(|s| s.contains("本体の形式のまま"))
        );
        rig.h.release(b);
        rig.frame();
        assert_eq!(rig.doc.template, before);
        assert!(!rig.doc.dirty);
    }

    #[test]
    fn alt_drag_duplicates_selection() {
        let mut rig = Rig::new(two_captions_and_picture());
        rig.h.hold(Modifiers::ALT);
        let (a, b) = (rig.at(1.5, 1.25), rig.at(1.5, 2.25));
        rig.drag(a, b);
        assert_eq!(rig.doc.template.objects.len(), 4);
        assert_rect(rig.rect_of(0), [1.0, 1.0, 2.0, 1.5]);
        assert_rect(rig.rect_of(3), [1.0, 2.0, 2.0, 2.5]);
        assert_eq!(rig.doc.selection, [3].into());
    }

    #[test]
    fn resize_with_handles() {
        // (掴むハンドルの位置, 動かし先, 期待する矩形)
        let cases = [
            ((2.0, 1.5), (2.5, 2.0), [1.0, 1.0, 2.5, 2.0]),
            ((1.0, 1.0), (0.5, 0.5), [0.5, 0.5, 2.0, 1.5]),
            ((1.5, 1.5), (1.5, 2.0), [1.0, 1.0, 2.0, 2.0]),
            ((2.0, 1.25), (3.0, 1.25), [1.0, 1.0, 3.0, 1.5]),
            ((1.0, 1.25), (0.5, 1.25), [0.5, 1.0, 2.0, 1.5]),
            ((1.5, 1.0), (1.5, 0.5), [1.0, 0.5, 2.0, 1.5]),
        ];
        for (grab, to, expected) in cases {
            let mut rig = Rig::new(vec![cap([1.0, 1.0, 2.0, 1.5])]);
            rig.doc.selection = [0].into();
            rig.frame();
            let (a, b) = (rig.at(grab.0, grab.1), rig.at(to.0, to.1));
            rig.drag(a, b);
            assert_rect(rig.rect_of(0), expected);
        }
    }

    #[test]
    fn resize_never_collapses_below_one_millimetre() {
        let mut rig = Rig::new(vec![cap([1.0, 1.0, 2.0, 1.5])]);
        rig.doc.selection = [0].into();
        rig.frame();
        let (a, b) = (rig.at(2.0, 1.5), rig.at(0.2, 0.2));
        rig.drag(a, b);
        let r = rig.rect_of(0);
        assert!(r[2] - r[0] >= 1.0 / MM_PER_INCH - 1e-6, "{r:?}");
        assert!(r[3] - r[1] >= 1.0 / MM_PER_INCH - 1e-6, "{r:?}");
    }

    #[test]
    fn resize_snaps_edge_to_neighbor() {
        let mut rig = Rig::new(two_captions_and_picture());
        rig.view.snap_objects = true;
        rig.doc.selection = [0].into();
        rig.frame();
        let (a, b) = (rig.at(2.0, 1.25), rig.at(2.97, 1.25));
        rig.drag(a, b);
        assert_rect(rig.rect_of(0), [1.0, 1.0, 3.0, 1.5]);
    }

    #[test]
    fn marquee_selects_enclosed_objects() {
        let mut rig = Rig::new(two_captions_and_picture());
        let (a, b) = (rig.at(0.5, 0.5), rig.at(4.5, 2.5));
        rig.drag(a, b);
        assert_eq!(rig.doc.selection, [0, 1].into());
        // Shift なら既存の選択に足す
        rig.h.hold(Modifiers::SHIFT);
        let (a, b) = (rig.at(4.8, 0.5), rig.at(6.5, 2.5));
        rig.drag(a, b);
        assert_eq!(rig.doc.selection, [0, 1, 2].into());
        // 何も囲まない範囲選択で選択が外れる
        rig.h.hold(Modifiers::NONE);
        let (a, b) = (rig.at(0.2, 3.0), rig.at(0.6, 3.5));
        rig.drag(a, b);
        assert!(rig.doc.selection.is_empty());
    }

    #[test]
    fn tools_place_objects_by_dragging_and_return_to_select() {
        let cases = [
            (Tool::Picture, 1usize, 1usize),
            (Tool::Label, 1, 0),
            (Tool::Value, 1, 0),
            (Tool::Row, 2, 0),
        ];
        for (tool, added, pictures) in cases {
            let mut rig = Rig::new(vec![]);
            rig.tool = tool;
            let (a, b) = (rig.at(1.0, 1.0), rig.at(3.0, 1.5));
            rig.drag(a, b);
            assert_eq!(rig.doc.template.objects.len(), added, "{tool:?}");
            let n = rig
                .doc
                .template
                .objects
                .iter()
                .filter(|o| o.kind == KIND_PICTURE)
                .count();
            assert_eq!(n, pictures, "{tool:?}");
            assert_eq!(rig.tool, Tool::Select, "{tool:?}");
            assert!(rig.doc.dirty, "{tool:?}");
            if tool != Tool::Row {
                assert_rect(rig.rect_of(0), [1.0, 1.0, 3.0, 1.5]);
            }
        }
    }

    #[test]
    fn click_with_tool_places_default_sized_object() {
        let mut rig = Rig::new(vec![]);
        rig.tool = Tool::Picture;
        let p = rig.at(1.0, 1.0);
        rig.click(p);
        assert_eq!(rig.doc.template.objects.len(), 1);
        let (dw, dh) = default_size(Tool::Picture);
        let r = rig.rect_of(0);
        assert!((r[2] - r[0] - dw).abs() < 2e-3 && (r[3] - r[1] - dh).abs() < 2e-3);
        assert_eq!(rig.tool, Tool::Select);
    }

    #[test]
    fn placing_drag_adds_object_only_on_release() {
        let mut rig = Rig::new(vec![]);
        rig.tool = Tool::Value;
        let (a, b) = (rig.at(1.0, 1.0), rig.at(2.0, 1.5));
        rig.h.pointer_to(a);
        rig.frame();
        rig.h.press(a);
        rig.frame();
        rig.h.pointer_to(b);
        rig.frame();
        assert!(rig.doc.template.objects.is_empty());
        rig.h.release(b);
        rig.frame();
        assert_eq!(rig.doc.template.objects.len(), 1);
    }

    #[test]
    fn double_click_on_caption_requests_text_focus() {
        let mut rig = Rig::new(two_captions_and_picture());
        let p = rig.at(1.5, 1.25);
        rig.click(p);
        rig.click(p);
        assert!(rig.st.focus_text);
        assert_eq!(rig.doc.selection, [0].into());
        // 写真枠では何も起きない
        let mut rig = Rig::new(two_captions_and_picture());
        let p = rig.at(5.5, 1.5);
        rig.click(p);
        rig.click(p);
        assert!(!rig.st.focus_text);
    }

    #[test]
    fn middle_button_drag_pans_the_view() {
        let mut rig = Rig::new(two_captions_and_picture());
        let before = rig.view.pan;
        let (a, b, c) = (pos2(400.0, 400.0), pos2(450.0, 430.0), pos2(480.0, 450.0));
        rig.h.pointer_to(a);
        rig.frame();
        rig.h.press_button(a, egui::PointerButton::Middle);
        rig.frame();
        // 動かし始めたフレームの位置が起点になるので、さらに動かして初めて画面が動く
        rig.h.pointer_to(b);
        rig.frame();
        rig.h.pointer_to(c);
        rig.frame();
        rig.h.release_button(c, egui::PointerButton::Middle);
        rig.frame();
        let moved = rig.view.pan - before;
        assert!(moved.x > 0.0 && moved.y > 0.0, "{moved:?}");
        assert!(rig.doc.template.objects.len() == 3 && !rig.doc.dirty);
    }

    #[test]
    fn zoom_input_changes_zoom_around_pointer() {
        let mut rig = Rig::new(two_captions_and_picture());
        let before = rig.view.zoom;
        let p = rig.at(2.0, 1.0);
        rig.h.pointer_to(p);
        rig.frame();
        rig.h.push(egui::Event::Zoom(1.5));
        rig.frame();
        assert!(rig.view.zoom > before);
        let after = rig.at(2.0, 1.0);
        assert!((after - p).length() < 1.0, "ポインタ下の点は動かない");
        // 上限で止まる
        for _ in 0..20 {
            rig.h.push(egui::Event::Zoom(4.0));
            rig.frame();
        }
        assert!(rig.view.zoom <= 8.0);
    }

    #[test]
    fn secondary_click_selects_hit_object() {
        let mut rig = Rig::new(two_captions_and_picture());
        let p = rig.at(3.5, 1.25);
        rig.h.pointer_to(p);
        rig.frame();
        rig.h.press_button(p, egui::PointerButton::Secondary);
        rig.frame();
        rig.h.release_button(p, egui::PointerButton::Secondary);
        let action = rig.frame();
        assert!(matches!(action, CanvasAction::None));
        assert_eq!(rig.doc.selection, [1].into());
    }

    #[test]
    fn rendering_covers_every_object_kind() {
        let mut filled = tpl::CaptionBody::new("見出し", "写真タイトル", false, 12);
        filled.fmt[fmt_off::FILL] = 1;
        filled.border_style = None;
        let mut value = tpl::CaptionBody::new("", "撮影年月日", true, 12);
        value.fmt[fmt_off::HORIZONTAL] = 0;
        let objects = vec![
            pic([0.1, 0.1, 2.0, 1.5]),
            Object::new(Body::Caption(filled), [2.1, 0.1, 4.0, 0.4]),
            Object::new(Body::Caption(value), [4.1, 0.1, 5.0, 2.0]),
            locked([5.1, 0.1, 6.0, 1.0]),
        ];
        let mut rig = Rig::new(objects);
        for sample in [false, true] {
            rig.view.sample_values = sample;
            rig.doc.select_all();
            rig.frame();
        }
        rig.view.show_grid = false;
        rig.view.show_used = false;
        rig.frame();
    }
}
