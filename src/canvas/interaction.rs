//! 編集面での操作（押す・動かす・離す・クリック）の判断。
//!
//! egui に依存しない。座標はインチ、修飾キーは自前の `Mods` で受け取り、
//! 画面座標からの換算や px の許容幅の換算は呼び出し側（`show`）が行う。
//! 文書（`Document`）を書き換えるが、描画や入力の読み取りはしない。

use super::geometry::{Handle, bounds_of, norm};
use super::snap::{best_snap, candidates, snap, snap_point};
use super::state::Drag;
use super::tool::{TableSpec, Tool, default_size, place};
use crate::doc::{ClickMods, Document};
use crate::layout::CardFrame;
use crate::tpl::{Body, MM_PER_INCH};

/// インチ座標の点
pub(super) type Pt = (f64, f64);

#[derive(Clone, Copy, Default)]
pub(super) struct Mods {
    pub(super) shift: bool,
    pub(super) alt: bool,
    /// Ctrl（Mac は Cmd）
    pub(super) toggle: bool,
}

impl Mods {
    /// 選択を作り直さず、いまの選択に加える操作か（Shift か Ctrl）
    fn extends_selection(self) -> bool {
        self.shift || self.toggle
    }
}

/// 吸着の設定。`tol` は吸着が効く距離（インチ）
pub(super) struct SnapCfg {
    pub(super) grid: f64,
    pub(super) grid_on: bool,
    pub(super) objects_on: bool,
    pub(super) tol: f64,
}

/// 判断に必要な周辺の条件。許容幅は px をインチに換算した値
pub(super) struct Env<'a> {
    pub(super) frame: &'a CardFrame,
    pub(super) mods: Mods,
    pub(super) snap: SnapCfg,
    pub(super) hit_tol: f64,
    pub(super) handle_tol: f64,
    pub(super) table: &'a TableSpec,
}

/// 押した場所にあるもの
#[derive(Debug, PartialEq)]
pub(super) enum Target {
    Handle(usize, Handle),
    Object(usize),
    Empty,
}

/// 押したときの結果。`drag` が無ければドラッグは始まらない
pub(super) struct Press {
    pub(super) drag: Option<Drag>,
    pub(super) status: Option<String>,
}

/// ドラッグ中の 1 フレームの結果
#[derive(Default)]
pub(super) struct Fx {
    pub(super) guides_x: Vec<f64>,
    pub(super) guides_y: Vec<f64>,
    pub(super) status: Option<String>,
    /// 画面を動かす量（px）。`Pan` のときだけ
    pub(super) pan: Option<(f32, f32)>,
}

// ----- 押した場所の判定 -----

fn single_selected(doc: &Document) -> Option<usize> {
    match doc.selected_editable().as_slice() {
        [i] => Some(*i),
        _ => None,
    }
}

/// 点にある物（重なっていれば前面）
pub(super) fn hit_object(doc: &Document, p: Pt, tol: f64) -> Option<usize> {
    doc.template
        .objects
        .iter()
        .enumerate()
        .rev()
        .find_map(|(i, o)| {
            let n = o.normalized();
            (p.0 >= n[0] - tol && p.0 <= n[2] + tol && p.1 >= n[1] - tol && p.1 <= n[3] + tol)
                .then_some(i)
        })
}

/// 1 つだけ選んでいるときの、その大きさ変更ハンドル
fn handle_under(doc: &Document, p: Pt, tol: f64) -> Option<(usize, Handle)> {
    let i = single_selected(doc)?;
    let r = doc.template.objects[i].normalized();
    Handle::ALL
        .into_iter()
        .find(|h| {
            let (hx, hy) = h.at(r);
            (p.0 - hx).abs() <= tol && (p.1 - hy).abs() <= tol
        })
        .map(|h| (i, h))
}

/// 押した場所にあるものを調べる（ハンドルが物より優先）
pub(super) fn locate(doc: &Document, p: Pt, env: &Env) -> Target {
    if let Some((i, h)) = handle_under(doc, p, env.handle_tol) {
        Target::Handle(i, h)
    } else if let Some(i) = hit_object(doc, p, env.hit_tol) {
        Target::Object(i)
    } else {
        Target::Empty
    }
}

/// 配置の終点（方眼に吸着する設定なら寄せる）
pub(super) fn place_point(p: Pt, env: &Env) -> Pt {
    snap_point(p, env.snap.grid, env.snap.grid_on)
}

// ----- 押す -----

/// 左ボタンを押してドラッグが始まったときの遷移。`p0` は押した位置
pub(super) fn press(doc: &mut Document, tool: Tool, p0: Pt, env: &Env) -> Press {
    let mut out = Press {
        drag: None,
        status: None,
    };
    if tool != Tool::Select {
        out.drag = Some(Drag::Place {
            tool,
            start: place_point(p0, env),
        });
        return out;
    }
    match locate(doc, p0, env) {
        Target::Handle(idx, handle) => {
            doc.checkpoint(None);
            out.drag = Some(Drag::Resize {
                idx,
                handle,
                orig: doc.template.objects[idx].normalized(),
            });
        }
        Target::Object(i) => {
            if !doc.selection.contains(&i) {
                if !env.mods.extends_selection() {
                    doc.selection.clear();
                }
                doc.selection.insert(i);
                doc.set_anchor(i);
            }
            let origin: Vec<(usize, [f64; 4])> = doc
                .selected_editable()
                .into_iter()
                .map(|k| (k, doc.template.objects[k].rect))
                .collect();
            if origin.is_empty() {
                out.status = Some(
                    "この部品は本体の形式のまま保持しています（移動・変更はできません）".into(),
                );
            } else {
                doc.checkpoint(None);
                let origin = if env.mods.alt {
                    // Alt を押しながらドラッグで複製
                    let copies: Vec<_> = origin
                        .iter()
                        .map(|(k, _)| doc.template.objects[*k].clone())
                        .collect();
                    let start = doc.template.objects.len();
                    doc.template.objects.extend(copies);
                    doc.selection = (start..doc.template.objects.len()).collect();
                    doc.selection
                        .iter()
                        .map(|&k| (k, doc.template.objects[k].rect))
                        .collect()
                } else {
                    origin
                };
                out.drag = Some(Drag::Move { start: p0, origin });
            }
        }
        Target::Empty => {
            if !env.mods.extends_selection() {
                doc.selection.clear();
            }
            out.drag = Some(Drag::Marquee { start: p0 });
        }
    }
    out
}

// ----- 動かす -----

/// ドラッグ中の 1 フレーム。`px` は画面の位置、`p` はインチの位置
pub(super) fn drag(doc: &mut Document, d: &mut Drag, px: (f32, f32), p: Pt, env: &Env) -> Fx {
    let mut fx = Fx::default();
    match d {
        Drag::Pan { last } => {
            fx.pan = Some((px.0 - last.0, px.1 - last.1));
            *last = px;
        }
        Drag::Move { start, origin } => {
            let (mut dx, mut dy) = (p.0 - start.0, p.1 - start.1);
            if env.mods.shift {
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
            let (cx, cy) = candidates(doc, env.frame, &moving, env.snap.objects_on);
            let (sx, gx) = best_snap(
                &[moved_b[0], (moved_b[0] + moved_b[2]) / 2.0, moved_b[2]],
                &cx,
                env.snap.tol,
            );
            let (sy, gy) = best_snap(
                &[moved_b[1], (moved_b[1] + moved_b[3]) / 2.0, moved_b[3]],
                &cy,
                env.snap.tol,
            );
            if let Some(g) = gx {
                dx += sx;
                fx.guides_x.push(g);
            } else if env.snap.grid_on {
                dx += snap(moved_b[0], env.snap.grid) - moved_b[0];
            }
            if let Some(g) = gy {
                dy += sy;
                fx.guides_y.push(g);
            } else if env.snap.grid_on {
                dy += snap(moved_b[1], env.snap.grid) - moved_b[1];
            }
            for (k, r) in origin.iter() {
                doc.template.objects[*k].rect = [r[0] + dx, r[1] + dy, r[2] + dx, r[3] + dy];
            }
            fx.status = Some(format!(
                "移動 {:+.1} mm, {:+.1} mm",
                dx * MM_PER_INCH,
                dy * MM_PER_INCH
            ));
        }
        Drag::Resize { idx, handle, orig } => {
            let (mut x, mut y) = p;
            let (ml, mt, mr, mb) = handle.moves();
            let (cx, cy) = candidates(doc, env.frame, &[*idx], env.snap.objects_on);
            let (sx, gx) = best_snap(&[x], &cx, env.snap.tol);
            let (sy, gy) = best_snap(&[y], &cy, env.snap.tol);
            if let Some(g) = gx {
                x += sx;
                fx.guides_x.push(g);
            } else if env.snap.grid_on {
                x = snap(x, env.snap.grid);
            }
            if let Some(g) = gy {
                y += sy;
                fx.guides_y.push(g);
            } else if env.snap.grid_on {
                y = snap(y, env.snap.grid);
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
            fx.status = Some(format!(
                "幅 {:.1} mm × 高さ {:.1} mm",
                (r[2] - r[0]) * MM_PER_INCH,
                (r[3] - r[1]) * MM_PER_INCH
            ));
        }
        // 範囲選択と配置の途中は、文書を変えない（枠の表示は呼び出し側）
        Drag::Marquee { .. } | Drag::Place { .. } => {}
    }
    fx
}

// ----- 離す -----

/// 部品ごとの、向きをそろえた矩形（番号は部品の番号）
fn normalized_rects(doc: &Document) -> Vec<[f64; 4]> {
    doc.template
        .objects
        .iter()
        .map(|o| o.normalized())
        .collect()
}

/// `area` の中に、すっかり含まれる部品の番号
fn parts_inside(rects: &[[f64; 4]], area: [f64; 4]) -> Vec<usize> {
    rects
        .iter()
        .enumerate()
        .filter(|(_, n)| n[0] >= area[0] && n[2] <= area[2] && n[1] >= area[1] && n[3] <= area[3])
        .map(|(i, _)| i)
        .collect()
}

/// ドラッグを終えたときの遷移。`p` は離した位置（読み取れなければ `None`）
pub(super) fn release(doc: &mut Document, d: Drag, p: Option<Pt>, tool: &mut Tool, env: &Env) {
    match d {
        Drag::Marquee { start } => {
            if let Some(p) = p {
                let (a, b) = (
                    (start.0.min(p.0), start.1.min(p.1)),
                    (start.0.max(p.0), start.1.max(p.1)),
                );
                let rects = normalized_rects(doc);
                doc.selection
                    .extend(parts_inside(&rects, [a.0, a.1, b.0, b.1]));
            }
        }
        Drag::Place { tool: t, start } => {
            if let Some(p) = p {
                place(doc, t, start, place_point(p, env), env.table);
                *tool = Tool::Select;
            }
        }
        Drag::Move { .. } | Drag::Resize { .. } | Drag::Pan { .. } => doc.end_coalesce(),
    }
}

// ----- クリック -----

/// 左クリック。ツールを選んでいれば既定の大きさで置き、そうでなければ選択を切り替える
pub(super) fn click(doc: &mut Document, tool: &mut Tool, p: Pt, env: &Env) {
    if *tool != Tool::Select {
        let s = place_point(p, env);
        let (w, h) = default_size(*tool, env.table);
        place(doc, *tool, s, (s.0 + w, s.1 + h), env.table);
        *tool = Tool::Select;
    } else if let Some(i) = hit_object(doc, p, env.hit_tol) {
        let mods = ClickMods {
            shift: env.mods.shift,
            toggle: env.mods.toggle,
        };
        // Shift+クリックは、起点の部品と押した部品を含む矩形に、すっかり含まれる部品を選ぶ（矩形選択と同じ規則）
        let rects = normalized_rects(doc);
        doc.click_select_by(&[i], mods, |anchor| {
            let span = bounds_of([*rects.get(anchor)?, *rects.get(i)?].into_iter());
            Some(parts_inside(&rects, span))
        });
    } else if !env.mods.extends_selection() {
        doc.selection.clear();
    }
}

/// ダブルクリック。文字項目の上なら選んで、文字欄を編集する合図（`true`）を返す
pub(super) fn double_click(doc: &mut Document, p: Pt, env: &Env) -> bool {
    match hit_object(doc, p, env.hit_tol) {
        Some(i) if matches!(doc.template.objects[i].body, Body::Caption(_)) => {
            doc.selection = [i].into();
            true
        }
        _ => false,
    }
}

/// 右クリック。選んでいない物の上なら、その物だけを選ぶ
pub(super) fn secondary_click(doc: &mut Document, p: Pt, env: &Env) {
    if let Some(i) = hit_object(doc, p, env.hit_tol)
        && !doc.selection.contains(&i)
    {
        doc.selection = [i].into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::card_frame;
    use crate::tpl::{CaptionBody, Object, PictureBody, Template};

    const NO_MODS: Mods = Mods {
        shift: false,
        alt: false,
        toggle: false,
    };
    const SHIFT: Mods = Mods {
        shift: true,
        alt: false,
        toggle: false,
    };
    const CTRL: Mods = Mods {
        shift: false,
        alt: false,
        toggle: true,
    };
    const CTRL_SHIFT: Mods = Mods {
        shift: true,
        alt: false,
        toggle: true,
    };
    const ALT: Mods = Mods {
        shift: false,
        alt: true,
        toggle: false,
    };

    fn cap(rect: [f64; 4]) -> Object {
        Object::new(Body::Caption(CaptionBody::new("a", "", false, 12)), rect)
    }

    fn pic(rect: [f64; 4]) -> Object {
        Object::new(Body::Picture(PictureBody::default()), rect)
    }

    fn locked(rect: [f64; 4]) -> Object {
        Object {
            tag_version: 3,
            kind: crate::tpl::KIND_RECT,
            link: 0,
            rect,
            body: Body::Passthrough(vec![0; 81]),
            trailer: Some(0),
        }
    }

    /// 履歴も選択も未保存の印も無い文書
    fn doc_of(objects: Vec<Object>) -> Document {
        Document::new(
            Template {
                objects,
                ..Template::default()
            },
            "t",
            None,
        )
    }

    /// 100 ppi 相当の許容幅（hit 0.03in, ハンドル 0.065in, 吸着 0.07in）。吸着は切ってある
    fn env_with(frame: &CardFrame, mods: Mods) -> Env<'_> {
        Env {
            frame,
            mods,
            snap: SnapCfg {
                grid: 0.1,
                grid_on: false,
                objects_on: false,
                tol: 0.07,
            },
            hit_tol: 0.03,
            handle_tol: 0.065,
            table: Box::leak(Box::new(TableSpec::default())),
        }
    }

    fn two_and_picture() -> Vec<Object> {
        vec![
            cap([1.0, 1.0, 2.0, 1.5]),
            cap([3.0, 1.0, 4.0, 1.5]),
            pic([5.0, 1.0, 6.0, 2.0]),
        ]
    }

    /// (押す前の選択, 修飾キー, 押した後の選択, 動かす物)
    type PressCase<'a> = (&'a [usize], Mods, &'a [usize], &'a [usize]);
    /// (押す前の選択, 位置, 修飾キー, 押した後の選択)
    type ClickCase<'a> = (&'a [usize], Pt, Mods, &'a [usize]);

    fn move_drag(origin: &[(usize, [f64; 4])], start: Pt) -> Drag {
        Drag::Move {
            start,
            origin: origin.to_vec(),
        }
    }

    fn assert_rect(actual: [f64; 4], expected: [f64; 4]) {
        for k in 0..4 {
            assert!(
                (actual[k] - expected[k]).abs() < 1e-9,
                "{actual:?} != {expected:?}"
            );
        }
    }

    // ----- 押した場所の判定 -----

    #[test]
    fn hit_object_prefers_front_and_uses_tolerance() {
        let doc = doc_of(vec![cap([1.0, 1.0, 3.0, 3.0]), cap([2.0, 2.0, 4.0, 4.0])]);
        // (点, 許容幅, 期待)
        let cases = [
            ((2.5, 2.5), 0.0, Some(1)),
            ((1.5, 1.5), 0.0, Some(0)),
            ((0.5, 0.5), 0.0, None),
            ((0.98, 1.5), 0.03, Some(0)),
            ((0.96, 1.5), 0.03, None),
            ((4.03, 4.0), 0.03, Some(1)),
        ];
        for (p, tol, expected) in cases {
            assert_eq!(hit_object(&doc, p, tol), expected, "{p:?} {tol}");
        }
    }

    #[test]
    fn hit_object_normalizes_reversed_rects() {
        let doc = doc_of(vec![cap([3.0, 3.0, 1.0, 1.0])]);
        assert_eq!(hit_object(&doc, (2.0, 2.0), 0.0), Some(0));
    }

    #[test]
    fn locate_prefers_handle_of_the_single_selected_object() {
        let frame = card_frame(&Template::default());
        let env = env_with(&frame, NO_MODS);
        let mut doc = doc_of(two_and_picture());
        // 何も選んでいなければハンドルは無く、物だけ
        assert_eq!(locate(&doc, (2.0, 1.5), &env), Target::Object(0));
        assert_eq!(locate(&doc, (2.5, 3.0), &env), Target::Empty);
        doc.selection = [0].into();
        assert_eq!(
            locate(&doc, (2.0, 1.5), &env),
            Target::Handle(0, Handle::SE)
        );
        assert_eq!(
            locate(&doc, (1.0, 1.25), &env),
            Target::Handle(0, Handle::W)
        );
        // 選択物の外にはみ出したハンドルの領域も拾う
        assert_eq!(
            locate(&doc, (2.06, 1.5), &env),
            Target::Handle(0, Handle::SE)
        );
        assert_eq!(locate(&doc, (2.08, 1.5), &env), Target::Empty);
        // 2 つ選ぶとハンドルは出ない
        doc.selection = [0, 1].into();
        assert_eq!(locate(&doc, (2.0, 1.5), &env), Target::Object(0));
        // 保持のみの物を 1 つ選んでもハンドルは出ない
        let mut doc = doc_of(vec![locked([1.0, 1.0, 2.0, 2.0])]);
        doc.selection = [0].into();
        assert_eq!(locate(&doc, (2.0, 2.0), &env), Target::Object(0));
    }

    #[test]
    fn place_point_snaps_to_grid_only_when_enabled() {
        let frame = card_frame(&Template::default());
        let mut env = env_with(&frame, NO_MODS);
        assert_eq!(place_point((1.04, 2.06), &env), (1.04, 2.06));
        env.snap.grid_on = true;
        let (x, y) = place_point((1.04, 2.06), &env);
        assert!((x - 1.0).abs() < 1e-9 && (y - 2.1).abs() < 1e-9);
    }

    // ----- 押す -----

    #[test]
    fn press_with_a_tool_starts_placing() {
        let frame = card_frame(&Template::default());
        let mut env = env_with(&frame, NO_MODS);
        env.snap.grid_on = true;
        let mut doc = doc_of(two_and_picture());
        let out = press(&mut doc, Tool::Picture, (1.04, 2.06), &env);
        assert!(out.status.is_none());
        match out.drag {
            Some(Drag::Place { tool, start }) => {
                assert_eq!(tool, Tool::Picture);
                assert!((start.0 - 1.0).abs() < 1e-9 && (start.1 - 2.1).abs() < 1e-9);
            }
            _ => panic!("配置の操作になるはず"),
        }
        assert!(!doc.dirty, "押しただけでは文書を変えない");
    }

    #[test]
    fn press_on_a_handle_starts_resizing_with_one_undo_step() {
        let frame = card_frame(&Template::default());
        let env = env_with(&frame, NO_MODS);
        let mut doc = doc_of(two_and_picture());
        doc.selection = [0].into();
        let out = press(&mut doc, Tool::Select, (2.0, 1.5), &env);
        match out.drag {
            Some(Drag::Resize { idx, handle, orig }) => {
                assert_eq!((idx, handle), (0, Handle::SE));
                assert_rect(orig, [1.0, 1.0, 2.0, 1.5]);
            }
            _ => panic!("大きさ変更になるはず"),
        }
        assert!(doc.can_undo() && doc.dirty);
    }

    #[test]
    fn press_on_an_object_selects_and_prepares_a_move() {
        let frame = card_frame(&Template::default());
        // (押す前の選択, 修飾キー, 押した後の選択, 動かす物)
        let cases: [PressCase; 5] = [
            (&[], NO_MODS, &[0], &[0]),
            (&[1], NO_MODS, &[0], &[0]),
            // Shift・Ctrl は、選択に加えて、まとめて動かす
            (&[1], SHIFT, &[0, 1], &[0, 1]),
            (&[1], CTRL, &[0, 1], &[0, 1]),
            // すでに選んでいる物を押すと、選択はそのまま（まとめて動かす）
            (&[0, 1], NO_MODS, &[0, 1], &[0, 1]),
        ];
        for (before, mods, after, moving) in cases {
            let mut doc = doc_of(two_and_picture());
            doc.selection = before.iter().copied().collect();
            let env = env_with(&frame, mods);
            let out = press(&mut doc, Tool::Select, (1.5, 1.25), &env);
            assert_eq!(
                doc.selection,
                after.iter().copied().collect(),
                "{before:?} shift={} ctrl={}",
                mods.shift,
                mods.toggle
            );
            match out.drag {
                Some(Drag::Move { start, origin }) => {
                    assert_eq!(start, (1.5, 1.25));
                    let ks: Vec<usize> = origin.iter().map(|(k, _)| *k).collect();
                    assert_eq!(ks, moving);
                }
                _ => panic!("移動になるはず"),
            }
            assert!(doc.can_undo());
        }
    }

    #[test]
    fn press_on_a_locked_object_only_reports_status() {
        let frame = card_frame(&Template::default());
        let env = env_with(&frame, NO_MODS);
        let mut doc = doc_of(vec![locked([1.0, 1.0, 2.0, 2.0])]);
        let out = press(&mut doc, Tool::Select, (1.5, 1.5), &env);
        assert!(out.drag.is_none());
        assert!(
            out.status
                .as_deref()
                .is_some_and(|s| s.contains("本体の形式のまま"))
        );
        assert!(!doc.dirty && !doc.can_undo());
        assert_eq!(doc.selection, [0].into(), "選択はされる");
    }

    #[test]
    fn press_with_alt_duplicates_the_selection() {
        let frame = card_frame(&Template::default());
        let env = env_with(&frame, ALT);
        let mut doc = doc_of(two_and_picture());
        doc.selection = [0, 1].into();
        let out = press(&mut doc, Tool::Select, (1.5, 1.25), &env);
        assert_eq!(doc.template.objects.len(), 5);
        assert_eq!(doc.selection, [3, 4].into(), "複製した側を選ぶ");
        match out.drag {
            Some(Drag::Move { origin, .. }) => {
                let ks: Vec<usize> = origin.iter().map(|(k, _)| *k).collect();
                assert_eq!(ks, vec![3, 4]);
            }
            _ => panic!("移動になるはず"),
        }
        assert_rect(doc.template.objects[0].rect, [1.0, 1.0, 2.0, 1.5]);
    }

    #[test]
    fn press_on_empty_space_starts_marquee_and_clears_unless_shift_or_ctrl() {
        let frame = card_frame(&Template::default());
        for (mods, kept) in [
            (NO_MODS, vec![]),
            (SHIFT, vec![0usize]),
            (CTRL, vec![0usize]),
        ] {
            let env = env_with(&frame, mods);
            let mut doc = doc_of(two_and_picture());
            doc.selection = [0].into();
            let out = press(&mut doc, Tool::Select, (1.5, 3.0), &env);
            assert!(matches!(out.drag, Some(Drag::Marquee { start }) if start == (1.5, 3.0)));
            assert_eq!(doc.selection, kept.into_iter().collect());
            assert!(!doc.dirty);
        }
    }

    // ----- 動かす -----

    #[test]
    fn drag_move_applies_delta_to_all_and_reports_status() {
        let frame = card_frame(&Template::default());
        let env = env_with(&frame, NO_MODS);
        let mut doc = doc_of(two_and_picture());
        let origin = [
            (0, doc.template.objects[0].rect),
            (1, doc.template.objects[1].rect),
        ];
        let mut d = move_drag(&origin, (1.5, 1.25));
        let fx = drag(&mut doc, &mut d, (0.0, 0.0), (2.0, 1.55), &env);
        assert_rect(doc.template.objects[0].rect, [1.5, 1.3, 2.5, 1.8]);
        assert_rect(doc.template.objects[1].rect, [3.5, 1.3, 4.5, 1.8]);
        assert_rect(doc.template.objects[2].rect, [5.0, 1.0, 6.0, 2.0]);
        assert_eq!(fx.status.as_deref(), Some("移動 +12.7 mm, +7.6 mm"));
        assert!(fx.guides_x.is_empty() && fx.guides_y.is_empty() && fx.pan.is_none());
        // 元の位置からの差で決まるので、同じ点を続けて渡しても進み続けない
        drag(&mut doc, &mut d, (0.0, 0.0), (2.0, 1.55), &env);
        assert_rect(doc.template.objects[0].rect, [1.5, 1.3, 2.5, 1.8]);
    }

    #[test]
    fn drag_move_with_shift_locks_to_the_dominant_axis() {
        let frame = card_frame(&Template::default());
        let env = env_with(&frame, SHIFT);
        let cases = [
            ((2.0, 1.5), [1.5, 1.0, 2.5, 1.5]),
            ((1.6, 2.5), [1.0, 2.25, 2.0, 2.75]),
        ];
        for (to, expected) in cases {
            let mut doc = doc_of(two_and_picture());
            let origin = [(0, doc.template.objects[0].rect)];
            let mut d = move_drag(&origin, (1.5, 1.25));
            drag(&mut doc, &mut d, (0.0, 0.0), to, &env);
            assert_rect(doc.template.objects[0].rect, expected);
        }
    }

    #[test]
    fn drag_move_snaps_to_neighbors_with_guides_or_to_grid() {
        let frame = card_frame(&Template::default());
        let mut env = env_with(&frame, NO_MODS);
        env.snap.objects_on = true;
        let mut doc = doc_of(two_and_picture());
        let origin = [(0, doc.template.objects[0].rect)];
        let mut d = move_drag(&origin, (1.5, 1.25));
        // 右端 2.0 → 2.96。隣の左端 3.0 に吸い付き、ガイドを返す
        let fx = drag(&mut doc, &mut d, (0.0, 0.0), (2.46, 1.25), &env);
        assert_rect(doc.template.objects[0].rect, [2.0, 1.0, 3.0, 1.5]);
        assert_eq!(fx.guides_x, vec![3.0]);
        // 物に吸着しない所では方眼へ
        let mut env = env_with(&frame, NO_MODS);
        env.snap.grid_on = true;
        let mut doc = doc_of(vec![cap([1.0, 1.0, 2.0, 1.5])]);
        let origin = [(0, doc.template.objects[0].rect)];
        let mut d = move_drag(&origin, (1.5, 1.25));
        let fx = drag(&mut doc, &mut d, (0.0, 0.0), (1.74, 1.43), &env);
        let r = doc.template.objects[0].rect;
        assert!(((r[0] / 0.1).round() * 0.1 - r[0]).abs() < 1e-9, "{r:?}");
        assert!(((r[1] / 0.1).round() * 0.1 - r[1]).abs() < 1e-9, "{r:?}");
        assert!(fx.guides_x.is_empty() && fx.guides_y.is_empty());
    }

    #[test]
    fn drag_resize_moves_only_the_grabbed_edges() {
        let frame = card_frame(&Template::default());
        let env = env_with(&frame, NO_MODS);
        let orig = [1.0, 1.0, 2.0, 1.5];
        // (ハンドル, 動かし先, 期待)
        let cases = [
            (Handle::SE, (2.5, 2.2), [1.0, 1.0, 2.5, 2.2]),
            (Handle::NW, (0.5, 0.5), [0.5, 0.5, 2.0, 1.5]),
            (Handle::N, (9.0, 0.5), [1.0, 0.5, 2.0, 1.5]),
            (Handle::S, (9.0, 2.3), [1.0, 1.0, 2.0, 2.3]),
            (Handle::E, (3.0, 9.0), [1.0, 1.0, 3.0, 1.5]),
            (Handle::W, (0.5, 9.0), [0.5, 1.0, 2.0, 1.5]),
            (Handle::NE, (3.0, 0.5), [1.0, 0.5, 3.0, 1.5]),
            (Handle::SW, (0.5, 2.3), [0.5, 1.0, 2.0, 2.3]),
        ];
        for (handle, to, expected) in cases {
            let mut doc = doc_of(vec![cap(orig)]);
            let mut d = Drag::Resize {
                idx: 0,
                handle,
                orig,
            };
            let fx = drag(&mut doc, &mut d, (0.0, 0.0), to, &env);
            assert_rect(doc.template.objects[0].rect, expected);
            let r = doc.template.objects[0].rect;
            let want = format!(
                "幅 {:.1} mm × 高さ {:.1} mm",
                (r[2] - r[0]) * MM_PER_INCH,
                (r[3] - r[1]) * MM_PER_INCH
            );
            assert_eq!(fx.status.as_deref(), Some(want.as_str()), "{handle:?}");
        }
    }

    #[test]
    fn drag_resize_keeps_at_least_one_millimetre() {
        let frame = card_frame(&Template::default());
        let env = env_with(&frame, NO_MODS);
        let orig = [1.0, 1.0, 2.0, 1.5];
        let min = 1.0 / MM_PER_INCH;
        for (handle, to) in [(Handle::SE, (0.2, 0.2)), (Handle::NW, (9.0, 9.0))] {
            let mut doc = doc_of(vec![cap(orig)]);
            let mut d = Drag::Resize {
                idx: 0,
                handle,
                orig,
            };
            drag(&mut doc, &mut d, (0.0, 0.0), to, &env);
            let r = doc.template.objects[0].rect;
            assert!(
                r[2] - r[0] >= min - 1e-9 && r[3] - r[1] >= min - 1e-9,
                "{r:?}"
            );
        }
    }

    #[test]
    fn drag_resize_snaps_the_edge_and_returns_the_guide() {
        let frame = card_frame(&Template::default());
        let mut env = env_with(&frame, NO_MODS);
        env.snap.objects_on = true;
        let mut doc = doc_of(two_and_picture());
        let mut d = Drag::Resize {
            idx: 0,
            handle: Handle::E,
            orig: [1.0, 1.0, 2.0, 1.5],
        };
        let fx = drag(&mut doc, &mut d, (0.0, 0.0), (2.97, 1.25), &env);
        assert_rect(doc.template.objects[0].rect, [1.0, 1.0, 3.0, 1.5]);
        assert_eq!(fx.guides_x, vec![3.0]);
    }

    #[test]
    fn drag_pan_reports_screen_delta_and_remembers_last() {
        let frame = card_frame(&Template::default());
        let env = env_with(&frame, NO_MODS);
        let mut doc = doc_of(vec![]);
        let mut d = Drag::Pan { last: (10.0, 20.0) };
        let fx = drag(&mut doc, &mut d, (15.0, 18.0), (0.0, 0.0), &env);
        assert_eq!(fx.pan, Some((5.0, -2.0)));
        let fx = drag(&mut doc, &mut d, (15.0, 18.0), (0.0, 0.0), &env);
        assert_eq!(fx.pan, Some((0.0, 0.0)));
        assert!(!doc.dirty);
    }

    #[test]
    fn drag_marquee_and_place_leave_the_document_alone() {
        let frame = card_frame(&Template::default());
        let env = env_with(&frame, NO_MODS);
        let mut doc = doc_of(two_and_picture());
        let before = doc.template.clone();
        for mut d in [
            Drag::Marquee { start: (0.0, 0.0) },
            Drag::Place {
                tool: Tool::Label,
                start: (0.0, 0.0),
            },
        ] {
            let fx = drag(&mut doc, &mut d, (1.0, 1.0), (1.0, 1.0), &env);
            assert!(fx.status.is_none() && fx.pan.is_none());
        }
        assert_eq!(doc.template, before);
    }

    // ----- 離す -----

    #[test]
    fn release_marquee_adds_only_fully_enclosed_objects() {
        let frame = card_frame(&Template::default());
        let env = env_with(&frame, NO_MODS);
        let mut tool = Tool::Select;
        // 終点が始点より左上でも同じ範囲になる
        for (start, end) in [((0.5, 0.5), (4.5, 2.5)), ((4.5, 2.5), (0.5, 0.5))] {
            let mut doc = doc_of(two_and_picture());
            doc.selection = [2].into();
            release(
                &mut doc,
                Drag::Marquee { start },
                Some(end),
                &mut tool,
                &env,
            );
            assert_eq!(doc.selection, [0, 1, 2].into(), "既存の選択に足す");
        }
        // 部分的にかかっただけの物は含めない
        let mut doc = doc_of(two_and_picture());
        release(
            &mut doc,
            Drag::Marquee { start: (1.5, 0.5) },
            Some((4.5, 2.5)),
            &mut tool,
            &env,
        );
        assert_eq!(doc.selection, [1].into());
        // 離した位置が分からなければ何もしない
        let mut doc = doc_of(two_and_picture());
        release(
            &mut doc,
            Drag::Marquee { start: (0.0, 0.0) },
            None,
            &mut tool,
            &env,
        );
        assert!(doc.selection.is_empty());
    }

    #[test]
    fn release_place_adds_an_object_and_returns_to_select() {
        let frame = card_frame(&Template::default());
        let env = env_with(&frame, NO_MODS);
        let mut tool = Tool::Picture;
        let mut doc = doc_of(vec![]);
        let d = Drag::Place {
            tool: Tool::Picture,
            start: (1.0, 1.0),
        };
        release(&mut doc, d, Some((3.0, 2.0)), &mut tool, &env);
        assert_eq!(doc.template.objects.len(), 1);
        assert_rect(doc.template.objects[0].rect, [1.0, 1.0, 3.0, 2.0]);
        assert_eq!(tool, Tool::Select);
        // 離した位置が分からなければ置かず、ツールもそのまま
        let mut tool = Tool::Label;
        let mut doc = doc_of(vec![]);
        let d = Drag::Place {
            tool: Tool::Label,
            start: (1.0, 1.0),
        };
        release(&mut doc, d, None, &mut tool, &env);
        assert!(doc.template.objects.is_empty());
        assert_eq!(tool, Tool::Label);
    }

    #[test]
    fn release_of_move_resize_and_pan_ends_the_undo_coalescing() {
        let frame = card_frame(&Template::default());
        let env = env_with(&frame, NO_MODS);
        let mut tool = Tool::Select;
        let drags = [
            Drag::Move {
                start: (0.0, 0.0),
                origin: vec![],
            },
            Drag::Resize {
                idx: 0,
                handle: Handle::E,
                orig: [0.0; 4],
            },
            Drag::Pan { last: (0.0, 0.0) },
        ];
        for d in drags {
            let mut doc = doc_of(vec![cap([0.0, 0.0, 1.0, 1.0])]);
            doc.select_all();
            doc.move_selected(0.1, 0.0, Some("k"));
            doc.move_selected(0.1, 0.0, Some("k"));
            release(&mut doc, d, None, &mut tool, &env);
            doc.move_selected(0.1, 0.0, Some("k"));
            let mut steps = 0;
            while doc.can_undo() {
                doc.undo();
                steps += 1;
            }
            assert_eq!(steps, 2, "区切られて別の 1 段になる");
        }
    }

    // ----- クリック -----

    #[test]
    fn click_with_a_tool_places_default_size_at_the_point() {
        let frame = card_frame(&Template::default());
        let env = env_with(&frame, NO_MODS);
        let mut tool = Tool::Value;
        let mut doc = doc_of(vec![]);
        click(&mut doc, &mut tool, (1.0, 1.0), &env);
        assert_eq!(doc.template.objects.len(), 1);
        let (w, h) = default_size(Tool::Value, &TableSpec::default());
        assert_rect(doc.template.objects[0].rect, [1.0, 1.0, 1.0 + w, 1.0 + h]);
        assert_eq!(tool, Tool::Select);
    }

    #[test]
    fn click_selection_rules() {
        let frame = card_frame(&Template::default());
        // (押す前の選択, 位置, 修飾キー, 押した後の選択)。選択は直接入れているので、起点はない
        let cases: [ClickCase; 10] = [
            (&[], (1.5, 1.25), NO_MODS, &[0]),
            (&[0], (3.5, 1.25), NO_MODS, &[1]),
            // Ctrl は、その部品だけを選択に加える・外す
            (&[0], (3.5, 1.25), CTRL, &[0, 1]),
            (&[0, 1], (3.5, 1.25), CTRL, &[0]),
            (&[0, 1], (3.5, 1.25), NO_MODS, &[1]),
            // 何も無い所: 修飾なしは選択を外す。Shift・Ctrl は、選択を保つ
            (&[0], (1.5, 3.0), NO_MODS, &[]),
            (&[0], (1.5, 3.0), CTRL, &[0]),
            (&[0], (1.5, 3.0), SHIFT, &[0]),
            // 起点がないときの Shift は、ふつうのクリック
            (&[0], (3.5, 1.25), SHIFT, &[1]),
            (&[0], (3.5, 1.25), CTRL_SHIFT, &[0, 1]),
        ];
        for (before, p, mods, after) in cases {
            let env = env_with(&frame, mods);
            let mut doc = doc_of(two_and_picture());
            doc.selection = before.iter().copied().collect();
            let mut tool = Tool::Select;
            click(&mut doc, &mut tool, p, &env);
            assert_eq!(
                doc.selection,
                after.iter().copied().collect(),
                "{before:?} {p:?} shift={} ctrl={}",
                mods.shift,
                mods.toggle
            );
            assert!(!doc.dirty, "選択は文書の変更ではない");
        }
    }

    /// 2×2 の表（0〜3）、離れた写真枠（4）、表の下の部品（5）
    fn table_and_picture() -> Vec<Object> {
        vec![
            cap([0.0, 0.0, 1.0, 0.5]),
            cap([1.0, 0.0, 2.0, 0.5]),
            cap([0.0, 0.5, 1.0, 1.0]),
            cap([1.0, 0.5, 2.0, 1.0]),
            pic([3.0, 0.0, 4.0, 2.0]),
            cap([0.0, 1.5, 1.0, 2.0]),
        ]
    }

    /// 部品の真ん中の点
    fn center(objects: &[Object], i: usize) -> Pt {
        let n = objects[i].normalized();
        ((n[0] + n[2]) / 2.0, (n[1] + n[3]) / 2.0)
    }

    #[test]
    fn shift_click_selects_the_parts_inside_the_span_from_the_anchor() {
        let frame = card_frame(&Template::default());
        // (名前, クリック列 (部品の番号, 修飾キー), 最後の選択)
        type Case<'a> = (&'a str, Vec<(usize, Mods)>, &'a [usize]);
        let cases: Vec<Case> = vec![
            (
                "起点と押した部品を対角とする矩形の中の部品",
                vec![(0, NO_MODS), (3, SHIFT)],
                &[0, 1, 2, 3],
            ),
            ("同じ行（上段）", vec![(0, NO_MODS), (1, SHIFT)], &[0, 1]),
            ("同じ列（左）", vec![(0, NO_MODS), (2, SHIFT)], &[0, 2]),
            (
                "逆向きにも伸ばせる",
                vec![(3, NO_MODS), (0, SHIFT)],
                &[0, 1, 2, 3],
            ),
            (
                "矩形で囲まれる 4 つだけ（離れた写真枠は含めない）",
                vec![(1, NO_MODS), (2, SHIFT)],
                &[0, 1, 2, 3],
            ),
            (
                "離れた部品までは、間の部品もまとめて",
                vec![(0, NO_MODS), (5, SHIFT)],
                &[0, 2, 5],
            ),
            (
                "起点は動かない（もう一度 Shift で、範囲だけ変わる）",
                vec![(0, NO_MODS), (3, SHIFT), (1, SHIFT)],
                &[0, 1],
            ),
            (
                "Ctrl で押した部品が起点になる",
                vec![(0, NO_MODS), (5, CTRL), (2, SHIFT)],
                &[2, 5],
            ),
            (
                "Ctrl+Shift は、範囲を選択に加える",
                vec![(0, NO_MODS), (5, CTRL), (2, CTRL_SHIFT)],
                &[0, 2, 5],
            ),
            (
                "起点がないときの Shift は、ふつうのクリック",
                vec![(3, SHIFT)],
                &[3],
            ),
        ];
        for (name, clicks, want) in cases {
            let objects = table_and_picture();
            let mut doc = doc_of(objects.clone());
            let mut tool = Tool::Select;
            for (i, mods) in clicks {
                click(
                    &mut doc,
                    &mut tool,
                    center(&objects, i),
                    &env_with(&frame, mods),
                );
            }
            assert_eq!(doc.selection, want.iter().copied().collect(), "{name}");
        }
    }

    #[test]
    fn double_click_focuses_text_only_for_captions() {
        let frame = card_frame(&Template::default());
        let env = env_with(&frame, NO_MODS);
        let mut doc = doc_of(two_and_picture());
        assert!(double_click(&mut doc, (3.5, 1.25), &env));
        assert_eq!(doc.selection, [1].into());
        let mut doc = doc_of(two_and_picture());
        assert!(!double_click(&mut doc, (5.5, 1.5), &env), "写真枠");
        assert!(doc.selection.is_empty());
        assert!(!double_click(&mut doc, (0.2, 3.0), &env), "何も無い所");
    }

    #[test]
    fn secondary_click_selects_unselected_object_only() {
        let frame = card_frame(&Template::default());
        let env = env_with(&frame, NO_MODS);
        let mut doc = doc_of(two_and_picture());
        doc.selection = [0].into();
        secondary_click(&mut doc, (3.5, 1.25), &env);
        assert_eq!(doc.selection, [1].into());
        // 選択済みの物の上では、複数選択を保つ
        doc.selection = [0, 1].into();
        secondary_click(&mut doc, (3.5, 1.25), &env);
        assert_eq!(doc.selection, [0, 1].into());
        // 何も無い所では変えない
        secondary_click(&mut doc, (0.2, 3.0), &env);
        assert_eq!(doc.selection, [0, 1].into());
    }
}
