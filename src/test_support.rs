//! テスト専用: ウィンドウも GPU も使わずに egui を 1 フレームずつ動かす道具。

use egui::{Context, Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Ui, pos2, vec2};

/// テストごとに作って、落ちるときに消す一時ディレクトリ
pub struct TempDir(std::path::PathBuf);

impl TempDir {
    pub fn new(tag: &str) -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static SEQ: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "cts-{tag}-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("一時ディレクトリを作れません");
        Self(dir)
    }

    pub fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub struct Harness {
    pub ctx: Context,
    pub screen: Rect,
    time: f64,
    events: Vec<Event>,
    modifiers: Modifiers,
}

impl Harness {
    pub fn new() -> Self {
        Self {
            ctx: Context::default(),
            screen: Rect::from_min_size(pos2(0.0, 0.0), vec2(1440.0, 900.0)),
            time: 0.0,
            events: Vec::new(),
            modifiers: Modifiers::NONE,
        }
    }

    /// 次のフレームから押した状態にする修飾キー
    pub fn hold(&mut self, modifiers: Modifiers) {
        if self.modifiers != modifiers {
            self.modifiers = modifiers;
            self.events.insert(0, Event::ModifiersChanged(modifiers));
        }
    }

    pub fn push(&mut self, e: Event) {
        self.events.push(e);
    }

    pub fn pointer_to(&mut self, p: Pos2) {
        self.push(Event::PointerMoved(p));
    }

    pub fn press(&mut self, p: Pos2) {
        self.press_button(p, PointerButton::Primary);
    }

    pub fn release(&mut self, p: Pos2) {
        self.release_button(p, PointerButton::Primary);
    }

    pub fn press_button(&mut self, p: Pos2, button: PointerButton) {
        self.push(Event::PointerMoved(p));
        self.push(Event::PointerButton {
            pos: p,
            button,
            pressed: true,
            modifiers: self.modifiers,
        });
    }

    pub fn release_button(&mut self, p: Pos2, button: PointerButton) {
        self.push(Event::PointerMoved(p));
        self.push(Event::PointerButton {
            pos: p,
            button,
            pressed: false,
            modifiers: self.modifiers,
        });
    }

    /// 時計だけ進める（連続クリックを double-click と見なさせないために使う）
    pub fn wait(&mut self, secs: f64) {
        self.time += secs;
    }

    /// 1 フレーム動かす。積んだ入力はこのフレームで使い切る
    pub fn frame(&mut self, mut ui_fn: impl FnMut(&mut Ui)) {
        let input = RawInput {
            screen_rect: Some(self.screen),
            time: Some(self.time),
            events: std::mem::take(&mut self.events),
            ..Default::default()
        };
        self.time += 1.0 / 60.0;
        self.ctx
            .run_ui(input, |ui| ui_fn(ui))
            .drop_without_applying_deltas();
    }
}
