//! カードの大きさと配置範囲。
//!
//! 本体のカード寸法の計算はまだ特定できていないため（MECHANICS.md の未確定事項）、
//! 手元の既存テンプレートの座標（OBJ の矩形）を実測した値を初期値として使う。

use crate::tpl::{A4_LONG_INCH, A4_SHORT_INCH, Header, Template};

/// 既存サンプルの OBJ 矩形の実測（インチ）。最小の左・上、最大の右・下
#[derive(Clone, Copy, Debug)]
pub struct LayoutRef {
    pub cols: u16,
    pub rows: u16,
    pub portrait: bool,
    pub min_left: f64,
    pub min_top: f64,
    pub max_right: f64,
    pub max_bottom: f64,
    pub samples: u32,
}

/// 2026-11-09 に TMPEXP の 70 テンプレートから測った値（mechanics.json と同じ）
pub const MEASURED: &[LayoutRef] = &[
    LayoutRef {
        cols: 1,
        rows: 1,
        portrait: false,
        min_left: 0.1,
        min_top: 2.029,
        max_right: 11.585,
        max_bottom: 7.902,
        samples: 2,
    },
    LayoutRef {
        cols: 1,
        rows: 2,
        portrait: true,
        min_left: 0.1,
        min_top: 0.17,
        max_right: 8.168,
        max_bottom: 5.681,
        samples: 2,
    },
    LayoutRef {
        cols: 1,
        rows: 3,
        portrait: true,
        min_left: 0.1,
        min_top: 0.1,
        max_right: 8.268,
        max_bottom: 3.898,
        samples: 56,
    },
    LayoutRef {
        cols: 1,
        rows: 4,
        portrait: true,
        min_left: 0.384,
        min_top: 0.1,
        max_right: 8.168,
        max_bottom: 2.672,
        samples: 1,
    },
    LayoutRef {
        cols: 2,
        rows: 1,
        portrait: false,
        min_left: 0.117,
        min_top: 1.763,
        max_right: 5.763,
        max_bottom: 7.968,
        samples: 2,
    },
    LayoutRef {
        cols: 2,
        rows: 2,
        portrait: false,
        min_left: 0.1,
        min_top: 0.691,
        max_right: 5.746,
        max_bottom: 3.946,
        samples: 4,
    },
    LayoutRef {
        cols: 4,
        rows: 5,
        portrait: true,
        min_left: 0.1,
        min_top: 0.1,
        max_right: 1.967,
        max_bottom: 2.223,
        samples: 2,
    },
    LayoutRef {
        cols: 5,
        rows: 5,
        portrait: true,
        min_left: 0.1,
        min_top: 0.1,
        max_right: 1.554,
        max_bottom: 1.916,
        samples: 1,
    },
];

pub fn measured(h: &Header) -> Option<&'static LayoutRef> {
    MEASURED
        .iter()
        .find(|m| m.cols == h.cols && m.rows == h.rows && m.portrait == h.is_portrait())
}

/// 画面に描くカードの枠
#[derive(Clone, Copy, Debug)]
pub struct CardFrame {
    /// カードの幅・高さ（インチ）
    pub width: f64,
    pub height: f64,
    /// 既存テンプレートで使われている配置範囲（インチ, left/top/right/bottom）
    pub used: Option<[f64; 4]>,
    /// 実測ではなく A4 を分割数で割った推定かどうか
    pub estimated: bool,
}

/// カード枠を決める。実測のあるレイアウトは「左・上の最小余白を右・下にも取った大きさ」、
/// 無いレイアウトは A4 ÷ 分割数（推定）。開いているテンプレートの物がはみ出すときは広げる。
pub fn card_frame(t: &Template) -> CardFrame {
    let (mut w, mut h, used, estimated) = match measured(&t.header) {
        Some(m) => (
            m.max_right + m.min_left,
            m.max_bottom + m.min_top,
            Some([m.min_left, m.min_top, m.max_right, m.max_bottom]),
            false,
        ),
        None => {
            let (w, h) = a4_division(&t.header);
            (w, h, None, true)
        }
    };
    for o in &t.objects {
        let [_, _, r, b] = o.normalized();
        w = w.max(r);
        h = h.max(b);
    }
    CardFrame {
        width: w,
        height: h,
        used,
        estimated,
    }
}

pub fn a4_division(h: &Header) -> (f64, f64) {
    let (pw, ph) = if h.is_portrait() {
        (A4_SHORT_INCH, A4_LONG_INCH)
    } else {
        (A4_LONG_INCH, A4_SHORT_INCH)
    };
    (pw / h.cols.max(1) as f64, ph / h.rows.max(1) as f64)
}

/// 新規作成で選べるレイアウト（本体の「１ページに横○枚、縦○枚」に合わせた候補）
pub const PRESETS: &[(u16, u16, bool)] = &[
    (1, 3, true),
    (1, 2, true),
    (1, 4, true),
    (1, 1, true),
    (2, 3, true),
    (3, 4, true),
    (4, 5, true),
    (5, 5, true),
    (1, 1, false),
    (2, 1, false),
    (2, 2, false),
];

pub fn layout_label(cols: u16, rows: u16, portrait: bool) -> String {
    format!(
        "横{cols}×縦{rows}（{}）",
        if portrait { "縦向き" } else { "横向き" }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tpl::{Body, CaptionBody, Object};

    #[test]
    fn frame_uses_measured_values_or_estimate() {
        let cases = [((1u16, 3u16, 1u32), false), ((3, 4, 1), true)];
        for ((cols, rows, orientation), estimated) in cases {
            let mut t = Template::default();
            t.header.cols = cols;
            t.header.rows = rows;
            t.header.orientation = orientation;
            let f = card_frame(&t);
            assert_eq!(f.estimated, estimated, "{cols}x{rows}");
            assert!(f.width > 0.0 && f.height > 0.0);
        }
    }

    #[test]
    fn frame_grows_to_fit_objects() {
        let mut t = Template::default();
        t.objects.push(Object::new(
            Body::Caption(CaptionBody::new("a", "", false, 12)),
            [0.0, 0.0, 20.0, 9.0],
        ));
        let f = card_frame(&t);
        assert!(f.width >= 20.0 && f.height >= 9.0);
    }

    fn header(cols: u16, rows: u16, portrait: bool) -> Header {
        Header {
            cols,
            rows,
            orientation: portrait as u32,
            ..Header::default()
        }
    }

    #[test]
    fn measured_lookup_matches_layout_and_orientation() {
        let cases = [
            (1u16, 3u16, true, Some(56u32)),
            (1, 3, false, None),
            (2, 2, false, Some(4)),
            (2, 2, true, None),
            (9, 9, true, None),
        ];
        for (cols, rows, portrait, samples) in cases {
            let m = measured(&header(cols, rows, portrait));
            assert_eq!(m.map(|m| m.samples), samples, "{cols}x{rows} {portrait}");
        }
    }

    #[test]
    fn measured_frame_adds_the_margin_on_the_far_side() {
        let t = Template::default();
        let f = card_frame(&t);
        let m = measured(&t.header).unwrap();
        assert!((f.width - (m.max_right + m.min_left)).abs() < 1e-9);
        assert!((f.height - (m.max_bottom + m.min_top)).abs() < 1e-9);
        assert_eq!(
            f.used,
            Some([m.min_left, m.min_top, m.max_right, m.max_bottom])
        );
        assert!(!f.estimated);
    }

    #[test]
    fn a4_division_splits_the_sheet() {
        let (w, h) = a4_division(&header(2, 3, true));
        assert!((w - A4_SHORT_INCH / 2.0).abs() < 1e-9);
        assert!((h - A4_LONG_INCH / 3.0).abs() < 1e-9);
        let (w, h) = a4_division(&header(2, 3, false));
        assert!((w - A4_LONG_INCH / 2.0).abs() < 1e-9);
        assert!((h - A4_SHORT_INCH / 3.0).abs() < 1e-9);
        // 0 分割でも 0 除算にならない
        let (w, h) = a4_division(&header(0, 0, true));
        assert!(w.is_finite() && h.is_finite());
    }

    #[test]
    fn estimated_frame_has_no_used_range() {
        let f = card_frame(&Template {
            header: header(3, 4, true),
            ..Template::default()
        });
        assert!(f.estimated && f.used.is_none());
    }

    #[test]
    fn frame_normalizes_reversed_rects() {
        let mut t = Template::default();
        t.objects.push(Object::new(
            Body::Caption(CaptionBody::new("a", "", false, 12)),
            [30.0, 25.0, 0.0, 0.0],
        ));
        let f = card_frame(&t);
        assert!(f.width >= 30.0 && f.height >= 25.0);
    }

    #[test]
    fn presets_are_unique_and_labelled() {
        let set: std::collections::BTreeSet<_> = PRESETS.iter().collect();
        assert_eq!(set.len(), PRESETS.len());
        assert_eq!(layout_label(1, 3, true), "横1×縦3（縦向き）");
        assert_eq!(layout_label(2, 1, false), "横2×縦1（横向き）");
        for &(c, r, p) in PRESETS {
            assert!(c >= 1 && r >= 1, "{c}x{r} {p}");
        }
    }

    #[test]
    fn measured_values_are_sane() {
        for m in MEASURED {
            assert!(m.min_left < m.max_right && m.min_top < m.max_bottom);
            assert!(m.samples > 0);
        }
    }
}
