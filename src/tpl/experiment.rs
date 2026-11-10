//! 実機（本体）での差分実験用の、実物からの TPL 生成（Issue #18）。
//!
//! 実物の TMPEXP（`CARD_TPL_SAMPLES`）を土台に、調べたい値だけを変えた TPL を作る。
//! 手動実行専用（`#[ignore]`）。出力先は `CARD_TPL_EXPERIMENT_OUT`。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::*;

fn samples_dir() -> String {
    std::env::var("CARD_TPL_SAMPLES").expect("CARD_TPL_SAMPLES が未設定")
}

fn read_template(rel: &str) -> Template {
    let p = Path::new(&samples_dir()).join(rel);
    let data = std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    Template::parse(&data).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn write_template(out_dir: &Path, folder: &str, name: &str, t: &Template) -> PathBuf {
    let dir = out_dir.join(folder);
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(format!("{name}.TPL"));
    std::fs::write(&p, t.to_bytes()).unwrap();
    p
}

#[test]
#[ignore = "CARD_TPL_SAMPLES に実物の TMPEXP が要る。調査用の手動実行"]
fn dump_template() {
    let rel =
        std::env::var("CARD_TPL_DUMP").expect("CARD_TPL_DUMP（TMPEXP からの相対パス）が未設定");
    let t = read_template(&rel);
    println!("header: {:?} card_color={:#x}", t.header, t.card_color);
    for (i, o) in t.objects.iter().enumerate() {
        match &o.body {
            Body::Picture(p) => println!(
                "#{i} 写真枠 link={} rect={:?} b8={} bc={:#x} cc={:#x} d4={} d8={} e0={:#x} e8={} ec={:?}",
                o.link, o.rect, p.b8, p.bc, p.cc, p.d4, p.d8, p.e0, p.e8, p.ec
            ),
            Body::Caption(c) => println!(
                "#{i} 文字 link={} rect={:?} text={:?} field={:?} is_value={}",
                o.link,
                o.rect,
                c.text.to_string_lossy(),
                c.field.to_string_lossy(),
                c.is_value
            ),
            _ => println!("#{i} {} link={} rect={:?}", o.kind_label(), o.link, o.rect),
        }
    }
}

#[test]
#[ignore = "CARD_TPL_SAMPLES に実物の TMPEXP が要る。調査用の手動実行"]
fn picture_combinations() {
    let mut h: BTreeMap<String, usize> = BTreeMap::new();
    let root = samples_dir();
    for sub in std::fs::read_dir(&root).unwrap().flatten() {
        if !sub.path().is_dir() {
            continue;
        }
        for f in std::fs::read_dir(sub.path()).unwrap().flatten() {
            let p = f.path();
            if p.extension().map(|e| e.eq_ignore_ascii_case("tpl")) != Some(true) {
                continue;
            }
            let t = Template::parse(&std::fs::read(&p).unwrap()).unwrap();
            for o in &t.objects {
                if let Body::Picture(pb) = &o.body {
                    let key = format!(
                        "b8={} bc={:#x} cc={:#x} e8={} 大きさ=({}, {})",
                        pb.b8, pb.bc, pb.cc, pb.e8, pb.d4, pb.d8
                    );
                    *h.entry(key).or_default() += 1;
                }
            }
        }
    }
    let mut rows: Vec<_> = h.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    for (k, n) in rows {
        println!("{n:>4}  {k}");
    }
}

/// 写真枠の `b8` / `bc` を枠ごとに変えた TPL（1x3 縦）を 1 つ作る。
/// 枠は左上から右へ 4 列 × 2 行に並べる（番号は並べた順）。
#[test]
#[ignore = "CARD_TPL_SAMPLES に実物の TMPEXP と、CARD_TPL_EXPERIMENT_OUT が要る。調査用の手動実行"]
fn gen_picture_frame_experiment() {
    let out = PathBuf::from(
        std::env::var("CARD_TPL_EXPERIMENT_OUT").expect("CARD_TPL_EXPERIMENT_OUT が未設定"),
    );
    // COLORREF は 0x00BBGGRR
    let cases: [(u32, u32); 8] = [
        (0, 0x0),
        (0x11, 0x0000ff),
        (0x11, 0x00ff00),
        (0x11, 0xff0000),
        (0x00, 0x0000ff),
        (0x10, 0x0000ff),
        (0x01, 0x0000ff),
        (0x02, 0x0000ff),
    ];
    let mut t = read_template("TEMP1x3_V/写真のみ.TPL");
    let base = t.objects.remove(0);
    let Body::Picture(base_body) = &base.body else {
        panic!("写真のみ.TPL の先頭が写真枠でない")
    };
    t.objects.clear();
    for (i, (b8, bc)) in cases.iter().enumerate() {
        let (col, row) = ((i % 4) as f64, (i / 4) as f64);
        let (x, y) = (0.1 + col * 2.0, 0.15 + row * 1.8);
        let mut o = base.clone();
        o.rect = [x, y, x + 1.85, y + 1.65];
        let mut b = base_body.clone();
        b.b8 = *b8;
        b.bc = *bc;
        o.body = Body::Picture(b);
        t.objects.push(o);
    }
    let p = write_template(&out, "TEMP1x3_V", "実験_写真枠b8bc", &t);
    println!("{}", p.display());
}
