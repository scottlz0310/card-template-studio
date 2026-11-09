use super::*;

use super::draw::*;
use super::geometry::*;
use super::snap::*;
use super::tool::*;
use super::view::*;
use crate::doc::Document;
use crate::layout::card_frame;
use crate::test_support::Harness;
use crate::tpl::{self, Body, MM_PER_INCH, Object, fmt_off};
use crate::tpl::{KIND_PICTURE, Template};
use egui::Modifiers;
use egui::{Color32, FontFamily, Pos2, Rect, pos2, vec2};

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
