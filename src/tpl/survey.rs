//! 実物テンプレートの値の分布を集計する（Issue #18 の調査用）。
//!
//! `CARD_TPL_SAMPLES` に TMPEXP フォルダを指定したときだけ意味を持つ。通常の
//! `cargo test` では `#[ignore]` で動かない。実行例:
//!
//! ```text
//! CARD_TPL_SAMPLES=<TMPEXP> cargo test survey_real_samples -- --ignored --nocapture
//! ```

use std::collections::BTreeMap;
use std::fmt::Debug;

use super::*;

type Hist = BTreeMap<String, usize>;

fn add(hists: &mut BTreeMap<String, Hist>, name: &str, value: impl Debug) {
    *hists
        .entry(name.to_string())
        .or_default()
        .entry(format!("{value:?}"))
        .or_default() += 1;
}

fn load(dir: &str) -> Vec<(String, Template)> {
    let mut out = Vec::new();
    for sub in std::fs::read_dir(dir).unwrap().flatten() {
        if !sub.path().is_dir() {
            continue;
        }
        for f in std::fs::read_dir(sub.path()).unwrap().flatten() {
            let p = f.path();
            if p.extension().map(|e| e.eq_ignore_ascii_case("tpl")) != Some(true) {
                continue;
            }
            let data = std::fs::read(&p).unwrap();
            let t = Template::parse(&data).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            out.push((p.display().to_string(), t));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn survey_caption(h: &mut BTreeMap<String, Hist>, c: &CaptionBody) {
    let f = &c.fmt;
    add(h, "文字 fmt+0x0a..0x0d", &f[0x0a..0x0e]);
    add(h, "文字 fmt+0x12", f[0x12]);
    add(h, "文字 fmt+0x13", f[0x13]);
    add(h, "文字 fmt+0x18 (i32)", read_fmt(f, 0x18) as i32);
    add(h, "文字 fmt+0x20", read_fmt(f, 0x20));
    add(h, "文字 fmt+0x24", read_fmt(f, 0x24));
    add(h, "文字 t108", c.t108);
    add(h, "文字 is_value", c.is_value);
    add(h, "文字 border_inset", c.border_inset);
    add(h, "文字 (t108, is_value)", (c.t108, c.is_value));
    add(
        h,
        "文字 (is_value, 項目名が空, 表示文字が空, 値の文字)",
        (
            c.is_value,
            c.field.is_empty(),
            c.text.is_empty(),
            c.value.to_string_lossy(),
        ),
    );
    add(
        h,
        "文字 (fmt+0x18, border)",
        (read_fmt(f, 0x18) as i32, c.border),
    );
}

fn survey_picture(h: &mut BTreeMap<String, Hist>, p: &PictureBody) {
    add(h, "写真枠 b8", p.b8);
    add(h, "写真枠 bc", p.bc);
    add(h, "写真枠 cc", p.cc);
    add(h, "写真枠 d0", p.d0);
    add(h, "写真枠 (d4, d8)", (p.d4, p.d8));
    add(h, "写真枠 e0", p.e0);
    add(h, "写真枠 e8", p.e8);
    add(h, "写真枠 ec", p.ec);
}

fn survey_cm(h: &mut BTreeMap<String, Hist>, c: &CmValueBody) {
    add(h, "施工管理値 c0", c.c0);
    add(h, "施工管理値 c4", c.c4);
    add(h, "施工管理値 x138", c.x138);
    add(h, "施工管理値 x13c", c.x13c);
    add(h, "施工管理値 x148", c.x148);
    add(h, "施工管理値 (x140, x144)", (c.x140, c.x144));
    add(h, "施工管理値 (x14c, x150, x154)", (c.x14c, c.x150, c.x154));
    add(
        h,
        "施工管理値 拡張の行数",
        c.ext.as_ref().map(|(_, _, rows)| rows.len()),
    );
}

#[test]
#[ignore = "CARD_TPL_SAMPLES に実物の TMPEXP が要る。調査用の手動実行"]
fn survey_real_samples() {
    let dir = std::env::var("CARD_TPL_SAMPLES").expect("CARD_TPL_SAMPLES が未設定");
    let templates = load(&dir);
    assert!(!templates.is_empty(), "サンプルが見つかりません");

    let mut h: BTreeMap<String, Hist> = BTreeMap::new();
    for (_, t) in &templates {
        add(&mut h, "ヘッダ enter_kind", t.header.enter_kind);
        add(&mut h, "ヘッダ sub_version", t.header.sub_version);
        add(&mut h, "ヘッダ card_color", t.card_color);
        add(
            &mut h,
            "ヘッダ (enter_kind, 向き, 列, 行)",
            (
                t.header.enter_kind,
                t.header.orientation,
                t.header.cols,
                t.header.rows,
            ),
        );
        for o in &t.objects {
            add(&mut h, "部品 種類", o.kind);
            add(&mut h, "部品 見出しの版", o.tag_version);
            add(&mut h, "部品 trailer", o.trailer);
            match &o.body {
                Body::Caption(c) => survey_caption(&mut h, c),
                Body::Picture(p) => survey_picture(&mut h, p),
                Body::CmValue(c) => survey_cm(&mut h, c),
                Body::Passthrough(_) => {}
            }
        }
    }

    println!("テンプレート {} 件", templates.len());
    for (name, hist) in &h {
        let total: usize = hist.values().sum();
        println!("\n## {name}（{total} 件）");
        let mut rows: Vec<_> = hist.iter().collect();
        rows.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        for (value, n) in rows {
            println!("  {n:>5}  {value}");
        }
    }
}

#[test]
#[ignore = "CARD_TPL_SAMPLES に実物の TMPEXP が要る。調査用の手動実行"]
fn list_odd_captions() {
    let dir = std::env::var("CARD_TPL_SAMPLES").expect("CARD_TPL_SAMPLES が未設定");
    for (path, t) in load(&dir) {
        for (i, o) in t.objects.iter().enumerate() {
            let Body::Caption(c) = &o.body else { continue };
            if c.is_value != -1 && c.t108 != -1 {
                continue;
            }
            println!(
                "{path} #{i} text={:?} value={:?} field={:?} t108={} is_value={} rect={:?}",
                c.text.to_string_lossy(),
                c.value.to_string_lossy(),
                c.field.to_string_lossy(),
                c.t108,
                c.is_value,
                o.rect
            );
        }
    }
}
