//! 吸着（方眼・ほかの部品・カードの端と中央）の計算。

use crate::doc::Document;
use crate::layout::CardFrame;

pub(super) fn snap(v: f64, grid: f64) -> f64 {
    if grid <= 0.0 {
        v
    } else {
        (v / grid).round() * grid
    }
}

pub(super) fn snap_point(p: (f64, f64), grid: f64, on: bool) -> (f64, f64) {
    if on {
        (snap(p.0, grid), snap(p.1, grid))
    } else {
        p
    }
}

/// 吸着先: 他の物の左・中央・右（上・中央・下）と、カードの端・中央
pub(super) fn candidates(
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
pub(super) fn best_snap(edges: &[f64], targets: &[f64], tol: f64) -> (f64, Option<f64>) {
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
