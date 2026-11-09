//! 画面を PNG に保存して終了する確認用の処理（--screenshot）。

use super::*;
use std::sync::{Arc, Mutex};

pub(super) struct Shot {
    pub(super) path: PathBuf,
    pub(super) frames: u32,
    pub(super) requested: bool,
    pub(super) image: Arc<Mutex<Option<Arc<egui::ColorImage>>>>,
}

impl StudioApp {
    pub(super) fn screenshot_tick(&mut self, ctx: &egui::Context) {
        let Some(shot) = &mut self.shot else { return };
        shot.frames += 1;
        ctx.request_repaint();
        if shot.frames >= 120 && !shot.requested {
            shot.requested = true;
            let slot = shot.image.clone();
            let ctx2 = ctx.clone();
            ctx.request_screenshot(move |img| {
                *slot.lock().unwrap() = Some(img);
                ctx2.request_repaint();
            });
        }
        let img = shot.image.lock().unwrap().take();
        if let Some(img) = img {
            let [w, h] = img.size;
            let mut rgba = Vec::with_capacity(w * h * 4);
            for p in &img.pixels {
                rgba.extend_from_slice(&p.to_srgba_unmultiplied());
            }
            if let Some(dir) = shot.path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Err(e) = image::save_buffer(
                &shot.path,
                &rgba,
                w as u32,
                h as u32,
                image::ColorType::Rgba8,
            ) {
                eprintln!("スクリーンショットを保存できません: {e}");
            }
            self.allow_close = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}
