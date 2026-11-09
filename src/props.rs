//! 右側のプロパティ欄。値を変えたら Document::checkpoint で「元に戻す」に積む。

use crate::canvas::ACCENT;
use crate::doc::{Document, KNOWN_FIELDS};
use crate::layout::{self, PRESETS, card_frame, layout_label};
use crate::tpl::{self, Body, Fmt, MM_PER_INCH, SjisString, fmt_off};
use egui::{Color32, RichText};

pub const FACES: &[&str] = &[
    "ＭＳ Ｐゴシック",
    "ＭＳ ゴシック",
    "ＭＳ Ｐ明朝",
    "ＭＳ 明朝",
];

/// 画面から変えてよい書式の位置と長さ。ここに無いバイト（意味が未確定）は決して書き換えない
const EDITABLE_FIELDS: &[(usize, usize)] = &[
    (fmt_off::ITALIC, 1),
    (fmt_off::STRIKE, 1),
    (fmt_off::UNDERLINE, 1),
    (fmt_off::HORIZONTAL, 1),
    (fmt_off::WRAP, 1),
    (fmt_off::ALIGN, 1),
    (fmt_off::FILL, 1),
    (fmt_off::SIZE, 2),
    (fmt_off::LINE_SPACE, 2),
    (fmt_off::CHAR_SPACE, 2),
    (fmt_off::WEIGHT, 4),
    (fmt_off::TEXT_COLOR, 4),
    (fmt_off::BG_COLOR, 4),
];

fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(title).strong().color(ACCENT));
    ui.separator();
}

fn mm_drag(ui: &mut egui::Ui, v: &mut f64, label: &str) -> bool {
    let mut mm = *v * MM_PER_INCH;
    let r = ui.add(
        egui::DragValue::new(&mut mm)
            .speed(0.1)
            .max_decimals(1)
            .prefix(label)
            .suffix(" mm"),
    );
    if r.changed() {
        *v = mm / MM_PER_INCH;
        true
    } else {
        false
    }
}

fn color_button(ui: &mut egui::Ui, c: &mut u32) -> bool {
    let mut rgb = tpl::colorref_rgb(*c);
    if ui.color_edit_button_srgb(&mut rgb).changed() {
        *c = tpl::rgb_colorref(rgb);
        true
    } else {
        false
    }
}

/// 一時的に編集した値を、変わったときだけ書き戻す
fn edit<T: Clone + PartialEq>(
    doc: &mut Document,
    key: &str,
    get: impl Fn(&Document) -> T,
    set: impl FnOnce(&mut Document, T),
    ui_fn: impl FnOnce(&mut T),
) {
    let before = get(doc);
    let mut v = before.clone();
    ui_fn(&mut v);
    if v != before {
        doc.checkpoint(Some(key));
        set(doc, v);
    }
}

pub fn show(ui: &mut egui::Ui, doc: &mut Document, focus_text: &mut bool) {
    let sel: Vec<usize> = doc.selection.iter().copied().collect();
    match sel.as_slice() {
        [] => card_props(ui, doc),
        [i] => object_props(ui, doc, *i, focus_text),
        many => {
            ui.label(RichText::new(format!("{} 個を選択中", many.len())).heading());
            ui.label("下の「配置」でまとめて揃えられます。共通の書式もここで変えられます。");
            multi_caption_props(ui, doc);
        }
    }
}

fn card_props(ui: &mut egui::Ui, doc: &mut Document) {
    ui.label(RichText::new("カード全体").heading());
    let frame = card_frame(&doc.template);
    section(ui, "レイアウト");
    let h = doc.template.header.clone();
    let mut choice = (h.cols, h.rows, h.is_portrait());
    egui::ComboBox::from_id_salt("layout")
        .width(220.0)
        .selected_text(layout_label(choice.0, choice.1, choice.2))
        .show_ui(ui, |ui| {
            for &(c, r, p) in PRESETS {
                ui.selectable_value(&mut choice, (c, r, p), layout_label(c, r, p));
            }
        });
    if choice != (h.cols, h.rows, h.is_portrait()) {
        doc.set_layout(choice.0, choice.1, choice.2);
    }
    ui.label(format!(
        "保存先フォルダ名: {}",
        doc.template.header.folder_name()
    ));
    ui.label(format!(
        "カードの大きさ: {:.1} × {:.1} mm{}",
        frame.width * MM_PER_INCH,
        frame.height * MM_PER_INCH,
        if frame.estimated {
            "（推定: A4÷分割数）"
        } else {
            "（既存テンプレートの実測から）"
        }
    ));
    if let Some(m) = layout::measured(&doc.template.header) {
        ui.label(
            RichText::new(format!(
                "緑の点線 = 既存テンプレート {} 個で使われていた配置範囲",
                m.samples
            ))
            .small()
            .color(Color32::from_rgb(0x2e, 0x7d, 0x57)),
        );
    }

    section(ui, "色");
    ui.horizontal(|ui| {
        ui.label("背景色");
        let mut c = doc.template.card_color;
        if color_button(ui, &mut c) {
            doc.checkpoint(Some("card-color"));
            doc.template.card_color = c;
        }
    });

    section(ui, "ファイルの情報");
    egui::Grid::new("hdr").num_columns(2).show(ui, |ui| {
        ui.label("形式の版");
        ui.label(format!(
            "{}.{}（保存時 {}.{}）",
            h.version,
            h.sub_version,
            h.version,
            doc.template.required_sub_version()
        ));
        ui.end_row();
        ui.label("入力区分");
        ui.label(h.enter_kind.to_string());
        ui.end_row();
        ui.label("部品の数");
        ui.label(doc.template.objects.len().to_string());
        ui.end_row();
    });
    let locked = doc
        .template
        .objects
        .iter()
        .filter(|o| o.is_passthrough())
        .count();
    if locked > 0 {
        ui.label(
            RichText::new(format!(
                "🔒 {locked} 個の図形・部品は本体の形式のまま保持しています（編集不可）"
            ))
            .small(),
        );
    }
}

fn geometry(ui: &mut egui::Ui, doc: &mut Document, i: usize) {
    section(ui, "位置と大きさ");
    let locked = doc.template.objects[i].is_passthrough();
    ui.add_enabled_ui(!locked, |ui| {
        let n = doc.template.objects[i].normalized();
        let mut x = n[0];
        let mut y = n[1];
        let mut w = n[2] - n[0];
        let mut h = n[3] - n[1];
        let mut changed = false;
        egui::Grid::new("geom")
            .num_columns(2)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                changed |= mm_drag(ui, &mut x, "X ");
                changed |= mm_drag(ui, &mut y, "Y ");
                ui.end_row();
                changed |= mm_drag(ui, &mut w, "幅 ");
                changed |= mm_drag(ui, &mut h, "高さ ");
                ui.end_row();
            });
        if changed {
            doc.checkpoint(Some(&format!("geom-{i}")));
            let min = 1.0 / MM_PER_INCH;
            doc.template.objects[i].rect = [x, y, x + w.max(min), y + h.max(min)];
        }
    });
}

fn object_props(ui: &mut egui::Ui, doc: &mut Document, i: usize, focus_text: &mut bool) {
    let label = doc.template.objects[i].kind_label();
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).heading());
        ui.label(RichText::new(format!("#{}", i + 1)).weak());
    });
    geometry(ui, doc, i);
    let pictures = doc
        .template
        .objects
        .iter()
        .filter(|o| o.kind == tpl::KIND_PICTURE)
        .count() as u32;
    match &doc.template.objects[i].body {
        Body::Caption(_) => caption_props(ui, doc, i, pictures, focus_text),
        Body::Picture(_) => {
            section(ui, "写真枠");
            let no = doc.template.objects[..=i]
                .iter()
                .filter(|o| o.kind == tpl::KIND_PICTURE)
                .count();
            ui.label(format!(
                "カード内の {no} 枚目の写真です。アルバムで写真を貼るとここに入ります。"
            ));
            ui.label(
                RichText::new("写真枠のその他の設定は、元のファイルの値をそのまま保持します。")
                    .small()
                    .weak(),
            );
        }
        Body::CmValue(_) => {
            section(ui, "施工管理値");
            ui.label("施工管理値の表です。位置と大きさを変えられます。中身の設定は元のファイルのまま保持します。");
        }
        Body::Passthrough(_) => {
            section(ui, "保持のみ");
            ui.label("🔒 この図形・部品は、本体で作られたままのバイト列を保持して書き戻します。");
            ui.label("位置・大きさ・中身は変更できません（本体側で編集してください）。");
        }
    }
}

fn caption_mut(doc: &mut Document, i: usize) -> &mut tpl::CaptionBody {
    match &mut doc.template.objects[i].body {
        Body::Caption(c) => c,
        _ => unreachable!("文字項目ではありません"),
    }
}

fn caption_ref(doc: &Document, i: usize) -> &tpl::CaptionBody {
    match &doc.template.objects[i].body {
        Body::Caption(c) => c,
        _ => unreachable!("文字項目ではありません"),
    }
}

fn caption_props(
    ui: &mut egui::Ui,
    doc: &mut Document,
    i: usize,
    pictures: u32,
    focus_text: &mut bool,
) {
    section(ui, "内容");
    edit(
        doc,
        &format!("role-{i}"),
        |d| caption_ref(d, i).is_value != 0,
        |d, v| caption_mut(d, i).is_value = v as i32,
        |v| {
            ui.horizontal(|ui| {
                ui.selectable_value(v, false, "項目名（固定の文字）");
                ui.selectable_value(v, true, "項目値（データを表示）");
            });
        },
    );
    let is_value = caption_ref(doc, i).is_value != 0;
    if !is_value {
        edit(
            doc,
            &format!("text-{i}"),
            |d| caption_ref(d, i).text.to_string_lossy(),
            |d, v| caption_mut(d, i).text = SjisString::from_str(&v),
            |v| {
                ui.label("表示する文字");
                let r = ui.add(
                    egui::TextEdit::multiline(v)
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
                if *focus_text {
                    r.request_focus();
                    *focus_text = false;
                }
                if tpl::sjis_unmappable(v) {
                    ui.colored_label(
                        Color32::from_rgb(0xc0, 0x39, 0x2b),
                        "本体で扱えない文字が含まれています（保存すると「?」になります）",
                    );
                }
            },
        );
    }
    edit(
        doc,
        &format!("field-{i}"),
        |d| caption_ref(d, i).field.to_string_lossy(),
        |d, v| caption_mut(d, i).field = SjisString::from_str(&v),
        |v| {
            ui.horizontal(|ui| {
                ui.label(if is_value {
                    "表示する項目"
                } else {
                    "対応する項目"
                });
                egui::ComboBox::from_id_salt(("field", i))
                    .width(150.0)
                    .selected_text(if v.is_empty() {
                        "（なし）".to_string()
                    } else {
                        v.clone()
                    })
                    .show_ui(ui, |ui| {
                        if !is_value {
                            ui.selectable_value(v, String::new(), "（なし）");
                        }
                        for f in KNOWN_FIELDS {
                            ui.selectable_value(v, (*f).to_string(), *f);
                        }
                    });
            });
            ui.add(
                egui::TextEdit::singleline(v)
                    .hint_text("項目名を直接入力")
                    .desired_width(f32::INFINITY),
            );
        },
    );
    if !is_value
        && caption_ref(doc, i).text.is_empty()
        && ui.small_button("項目名を表示する文字にする").clicked()
    {
        doc.checkpoint(None);
        let f = caption_ref(doc, i).field.clone();
        caption_mut(doc, i).text = f;
    }
    if pictures >= 2 {
        edit(
            doc,
            &format!("link-{i}"),
            |d| d.template.objects[i].link,
            |d, v| d.template.objects[i].link = v,
            |v| {
                ui.horizontal(|ui| {
                    ui.label("どの写真の情報か");
                    egui::ComboBox::from_id_salt(("link", i))
                        .selected_text(if *v == 0 {
                            "指定なし".into()
                        } else {
                            format!("写真 {v}")
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(v, 0, "指定なし");
                            for n in 1..=pictures {
                                ui.selectable_value(v, n, format!("写真 {n}"));
                            }
                        });
                });
            },
        );
    }

    section(ui, "文字");
    font_props(ui, doc, &[i]);

    section(ui, "枠線");
    edit(
        doc,
        &format!("border-{i}"),
        |d| {
            let c = caption_ref(d, i);
            (c.border != 0, c.border_style)
        },
        |d, (on, style)| {
            let c = caption_mut(d, i);
            c.border = on as i32;
            c.border_style = style;
        },
        |(on, style)| {
            ui.checkbox(on, "枠線を引く");
            if *on {
                match style {
                    Some((col, w)) => {
                        ui.horizontal(|ui| {
                            ui.label("色");
                            color_button(ui, col);
                            ui.label("太さ");
                            for (k, mm) in tpl::BORDER_WIDTHS_MM.iter().enumerate() {
                                ui.selectable_value(w, k as i32, format!("{mm} mm"));
                            }
                        });
                    }
                    None => {
                        ui.label(
                            RichText::new("古い形式のため枠線の色・太さは保存されません。")
                                .small()
                                .weak(),
                        );
                    }
                }
            }
        },
    );
}

/// 文字の書式（複数選択にも使う）。最初の物の値を見せ、変えたら全部に当てる
fn font_props(ui: &mut egui::Ui, doc: &mut Document, idx: &[usize]) {
    let captions: Vec<usize> = idx
        .iter()
        .copied()
        .filter(|&i| matches!(doc.template.objects[i].body, Body::Caption(_)))
        .collect();
    let Some(&first) = captions.first() else {
        ui.label("文字項目が選ばれていません。");
        return;
    };
    let key = format!("font-{captions:?}");
    let before = {
        let c = caption_ref(doc, first);
        (tpl::fixed_to_string(&c.face), c.fmt)
    };
    let (mut face, mut fmt) = before.clone();
    {
        let mut f = Fmt(&mut fmt);
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt(("face", first))
                .width(140.0)
                .selected_text(face.clone())
                .show_ui(ui, |ui| {
                    for name in FACES {
                        ui.selectable_value(&mut face, (*name).to_string(), *name);
                    }
                });
            let mut size = f.u16(fmt_off::SIZE);
            if ui
                .add(egui::DragValue::new(&mut size).range(4..=96).suffix(" pt"))
                .changed()
            {
                f.set_u16(fmt_off::SIZE, size);
            }
        });
        ui.horizontal(|ui| {
            let mut bold = f.u32(fmt_off::WEIGHT) as i32 > 400;
            if ui
                .toggle_value(&mut bold, RichText::new(" B ").strong())
                .on_hover_text("太字")
                .changed()
            {
                f.set_u32(fmt_off::WEIGHT, if bold { 700 } else { 400 });
            }
            for (off, text, tip) in [
                (fmt_off::ITALIC, " I ", "斜体"),
                (fmt_off::UNDERLINE, " U ", "下線"),
                (fmt_off::STRIKE, " S ", "取り消し線"),
            ] {
                let mut v = f.flag(off);
                let rt = match off {
                    fmt_off::ITALIC => RichText::new(text).italics(),
                    fmt_off::UNDERLINE => RichText::new(text).underline(),
                    _ => RichText::new(text).strikethrough(),
                };
                if ui.toggle_value(&mut v, rt).on_hover_text(tip).changed() {
                    f.set_flag(off, v);
                }
            }
            ui.separator();
            let mut vertical = !f.flag(fmt_off::HORIZONTAL);
            if ui.toggle_value(&mut vertical, "縦書き").changed() {
                f.set_flag(fmt_off::HORIZONTAL, !vertical);
            }
        });
        ui.horizontal(|ui| {
            ui.label("行揃え");
            let mut a = f.u8(fmt_off::ALIGN);
            for (v, t) in [(0u8, "左"), (1, "中央"), (2, "右")] {
                if ui.selectable_value(&mut a, v, t).changed() {
                    f.set_u8(fmt_off::ALIGN, a);
                }
            }
            let mut wrap = f.flag(fmt_off::WRAP);
            if ui.checkbox(&mut wrap, "自動折り返し").changed() {
                f.set_flag(fmt_off::WRAP, wrap);
            }
        });
        ui.horizontal(|ui| {
            let mut ls = f.u16(fmt_off::LINE_SPACE);
            if ui
                .add(
                    egui::DragValue::new(&mut ls)
                        .range(0..=300)
                        .prefix("行間 ")
                        .suffix(" %"),
                )
                .changed()
            {
                f.set_u16(fmt_off::LINE_SPACE, ls);
            }
            let mut cs = f.u16(fmt_off::CHAR_SPACE);
            if ui
                .add(
                    egui::DragValue::new(&mut cs)
                        .range(0..=300)
                        .prefix("文字間 ")
                        .suffix(" %"),
                )
                .changed()
            {
                f.set_u16(fmt_off::CHAR_SPACE, cs);
            }
        });
        ui.horizontal(|ui| {
            ui.label("文字色");
            let mut c = f.u32(fmt_off::TEXT_COLOR);
            if color_button(ui, &mut c) {
                f.set_u32(fmt_off::TEXT_COLOR, c);
            }
            ui.separator();
            let mut fill = f.flag(fmt_off::FILL);
            if ui.checkbox(&mut fill, "背景色").changed() {
                f.set_flag(fmt_off::FILL, fill);
            }
            let mut bg = f.u32(fmt_off::BG_COLOR);
            if color_button(ui, &mut bg) {
                f.set_u32(fmt_off::BG_COLOR, bg);
                f.set_flag(fmt_off::FILL, true);
            }
        });
    }
    if (face.clone(), fmt) != before {
        doc.checkpoint(Some(&key));
        // 変わった項目だけを全員に当てる（ほかの値・未解明のバイトはそれぞれのまま）
        let changed: Vec<(usize, usize)> = EDITABLE_FIELDS
            .iter()
            .copied()
            .filter(|&(off, len)| fmt[off..off + len] != before.1[off..off + len])
            .collect();
        let face_changed = face != before.0;
        for &i in &captions {
            let c = caption_mut(doc, i);
            for &(off, len) in &changed {
                c.fmt[off..off + len].copy_from_slice(&fmt[off..off + len]);
            }
            if face_changed {
                c.face = tpl::string_to_fixed(&face);
            }
        }
    }
}

fn multi_caption_props(ui: &mut egui::Ui, doc: &mut Document) {
    let sel: Vec<usize> = doc.selected_editable();
    if sel
        .iter()
        .any(|&i| matches!(doc.template.objects[i].body, Body::Caption(_)))
    {
        section(ui, "文字（まとめて変更）");
        font_props(ui, doc, &sel);
    }
}
