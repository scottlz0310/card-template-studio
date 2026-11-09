//! 編集面の 1 フレーム：入力を読み、文書を更新し、描く。

use super::draw::*;
use super::geometry::*;
use super::snap::*;
use super::state::*;
use super::tool::*;
use super::view::*;
use super::*;
use crate::doc::Document;
use crate::layout::card_frame;
use crate::tpl::{self, Body, MM_PER_INCH, Object};
use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};

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
