#![cfg_attr(not(test), windows_subsystem = "windows")]

mod app;
mod canvas;
mod doc;
mod layout;
mod layout_picker;
mod props;
mod settings;
#[cfg(test)]
mod test_support;
mod tpl;

use std::path::PathBuf;
use std::sync::Arc;

fn main() -> eframe::Result<()> {
    let mut screenshot: Option<PathBuf> = None;
    let mut open: Option<PathBuf> = None;
    let mut args = std::env::args_os().skip(1);
    while let Some(a) = args.next() {
        if a == "--screenshot" {
            screenshot = args.next().map(PathBuf::from);
        } else {
            open = Some(PathBuf::from(a));
        }
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("カードテンプレート工房")
        .with_inner_size([1440.0, 900.0])
        .with_min_inner_size([960.0, 600.0]);
    if let Some(icon) = load_app_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        // ウィンドウの大きさ・位置・最大化は、eframe の永続化に任せる（画面外の位置の補正を含む）。
        // 確認用の画面（--screenshot）は、利用者の保存済みの位置を書き換えない
        persist_window: screenshot.is_none(),
        ..Default::default()
    };
    // 名前は、eframe の保存先（%APPDATA%\<名前>\data\app.ron）になる。設定ファイルと同じフォルダに揃える
    eframe::run_native(
        settings::APP_DIR,
        options,
        Box::new(move |cc| {
            install_fonts(&cc.egui_ctx);
            Ok(Box::new(app::StudioApp::new(
                &cc.egui_ctx,
                open,
                screenshot,
                Box::new(settings::FileStore::in_app_data()),
            )))
        }),
    )
}

/// Windows に入っている日本語フォントを読み込む（フォント自体は同梱しない）。
/// カード上の文字は TPL のフォント名に近い書体で描く。
fn install_fonts(ctx: &egui::Context) {
    let dir = std::env::var_os("WINDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
        .join("Fonts");
    let mut fonts = egui::FontDefinitions::default();
    let mut load = |key: &str, file: &str, index: u32| -> bool {
        match std::fs::read(dir.join(file)) {
            Ok(bytes) => {
                fonts.font_data.insert(
                    key.to_owned(),
                    Arc::new(egui::FontData::from_blob(Arc::new(bytes), index)),
                );
                true
            }
            Err(_) => false,
        }
    };
    let ui_font = if load("ui", "YuGothM.ttc", 0)
        || load("ui", "meiryo.ttc", 0)
        || load("ui", "msgothic.ttc", 0)
    {
        Some("ui")
    } else {
        None
    };
    // ＭＳ ゴシック・ＭＳ 明朝（msgothic.ttc / msmincho.ttc）は egui の描画で字形が出ないため、
    // 画面表示だけ游ゴシック・游明朝で代用する（TPL に保存するフォント名は変えない）。
    // メイリオは字形が出るので、そのまま使う（本体の既定のフォント）
    let faces = [
        ("meiryo", "meiryo.ttc", 0),
        ("pgothic", "YuGothM.ttc", 0),
        ("gothic", "YuGothM.ttc", 0),
        ("pmincho", "yumin.ttf", 0),
        ("mincho", "yumin.ttf", 0),
    ];
    let mut loaded = Vec::new();
    for (key, file, index) in faces {
        if load(key, file, index) {
            loaded.push(key);
        }
    }
    if let Some(ui) = ui_font {
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts
                .families
                .entry(family)
                .or_default()
                .insert(0, ui.to_owned());
        }
    }
    for key in ["meiryo", "pgothic", "gothic", "pmincho", "mincho"] {
        let mut list = Vec::new();
        if loaded.contains(&key) {
            list.push(key.to_owned());
        }
        if let Some(ui) = ui_font {
            list.push(ui.to_owned());
        }
        list.extend(
            fonts
                .families
                .get(&egui::FontFamily::Proportional)
                .cloned()
                .unwrap_or_default(),
        );
        fonts
            .families
            .insert(egui::FontFamily::Name(key.into()), list);
    }
    ctx.set_fonts(fonts);
}

/// アプリアイコンを読み込む
fn load_app_icon() -> Option<egui::IconData> {
    const ICON_BYTES: &[u8] = include_bytes!("../assets/icon.png");
    match image::load_from_memory(ICON_BYTES) {
        Ok(image) => {
            let rgba = image.to_rgba8();
            let (width, height) = rgba.dimensions();
            Some(egui::IconData {
                rgba: rgba.into_raw(),
                width,
                height,
            })
        }
        Err(_) => None,
    }
}
