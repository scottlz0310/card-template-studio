//! 編集面の 1 フレーム：入力を読み、文書を更新し、描く。

use super::draw::*;
use super::geometry::Handle;
use super::interaction::{self, Env, Mods, SnapCfg, Target};
use super::state::*;
use super::tool::*;
use super::view::*;
use super::*;
use crate::doc::Document;
use crate::layout::card_frame;
use crate::tpl::{self, MM_PER_INCH};
use egui::{Align2, Color32, FontId, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};

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
        let (zoom, pan) = fit((work.width(), work.height()), (frame.width, frame.height));
        view.zoom = zoom;
        view.pan = vec2(pan.0, pan.1);
        view.fit_pending = false;
    }

    // ----- 拡大縮小とスクロール -----
    let hover = resp.hover_pos();
    if let Some(hp) = hover {
        let (zoom_delta, scroll) = ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta));
        if zoom_delta != 1.0 {
            let rel = hp - work.min;
            let (zoom, pan) = zoom_at(
                view.zoom,
                (view.pan.x, view.pan.y),
                zoom_delta,
                (rel.x, rel.y),
            );
            view.zoom = zoom;
            view.pan = vec2(pan.0, pan.1);
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
    // egui の値を、インチ座標・修飾キー・インチに換算した許容幅に直して interaction へ渡す
    let ppi = xf.ppi as f64;
    let shift = ui.input(|i| i.modifiers.shift);
    let alt = ui.input(|i| i.modifiers.alt);
    let toggle = ui.input(|i| i.modifiers.command);
    let space = ui.input(|i| i.key_down(egui::Key::Space));
    let single_sel: Option<usize> = match doc.selected_editable().as_slice() {
        [i] => Some(*i),
        _ => None,
    };
    let env = Env {
        frame: &frame,
        mods: Mods { shift, alt, toggle },
        snap: SnapCfg {
            grid: view.grid_mm as f64 / MM_PER_INCH,
            grid_on: view.snap_grid,
            objects_on: view.snap_objects,
            tol: SNAP_PX as f64 / ppi,
        },
        hit_tol: 3.0 / ppi,
        handle_tol: (HANDLE + 6.0) as f64 / 2.0 / ppi,
        table: &st.table,
    };
    let pointer = resp.interact_pointer_pos().or(hover);

    if let Some(p) = hover
        && st.drag.is_none()
    {
        if *tool != Tool::Select {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        } else {
            match interaction::locate(doc, xf.inch(p), &env) {
                Target::Handle(_, h) => ui.ctx().set_cursor_icon(h.cursor()),
                Target::Object(i) => {
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
                Target::Empty => {}
            }
        }
    }

    if resp.drag_started_by(egui::PointerButton::Middle) || (resp.drag_started() && space) {
        if let Some(p) = pointer {
            st.drag = Some(Drag::Pan { last: (p.x, p.y) });
        }
    } else if resp.drag_started_by(egui::PointerButton::Primary)
        && let Some(p) = resp.interact_pointer_pos()
    {
        let p0 = p - resp.drag_delta();
        let out = interaction::press(doc, *tool, xf.inch(p0), &env);
        if let Some(d) = out.drag {
            st.drag = Some(d);
        }
        if out.status.is_some() {
            st.status = out.status;
        }
    }

    st.guides_x.clear();
    st.guides_y.clear();
    if let Some(p) = pointer {
        if let Some(d) = st.drag.as_mut() {
            let fx = interaction::drag(doc, d, (p.x, p.y), xf.inch(p), &env);
            st.guides_x = fx.guides_x;
            st.guides_y = fx.guides_y;
            if fx.status.is_some() {
                st.status = fx.status;
            }
            if let Some((dx, dy)) = fx.pan {
                view.pan += vec2(dx, dy);
            }
        }
        match &st.drag {
            Some(Drag::Marquee { start }) => {
                let r = Rect::from_two_pos(xf.pt(start.0, start.1), p);
                clip.rect(
                    r,
                    0.0,
                    ACCENT.gamma_multiply(0.08),
                    Stroke::new(1.0, ACCENT),
                    StrokeKind::Inside,
                );
            }
            Some(Drag::Place { tool: t, start }) => {
                let e = interaction::place_point(xf.inch(p), &env);
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
            _ => {}
        }
    }

    if resp.drag_stopped() {
        match st.drag.take() {
            Some(d) => interaction::release(doc, d, pointer.map(|p| xf.inch(p)), tool, &env),
            // 押しただけで何も始まらなかったときも、まとめた「元に戻す」は区切る
            None => doc.end_coalesce(),
        }
        st.status = None;
    }

    if resp.clicked()
        && let Some(p) = resp.interact_pointer_pos()
    {
        interaction::click(doc, tool, xf.inch(p), &env);
    }
    if resp.double_clicked()
        && let Some(p) = resp.interact_pointer_pos()
        && interaction::double_click(doc, xf.inch(p), &env)
    {
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
    {
        interaction::secondary_click(doc, xf.inch(p), &env);
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
