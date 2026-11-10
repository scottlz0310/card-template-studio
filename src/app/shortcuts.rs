//! 編集の操作：複製とショートカットキー。

use super::*;

impl StudioApp {
    pub(super) fn duplicate(&mut self) {
        let objs = self.doc.copy_selected();
        self.doc.paste(&objs, 2.0 * MM);
    }

    pub(super) fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.modal.is_some() {
            return;
        }
        let cmd = |k| KeyboardShortcut::new(Modifiers::COMMAND, k);
        let cmd_shift = |k| KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, k);
        // egui の consume_shortcut は修飾キーが余分に押されていても一致するので、
        // Shift を含むものを先に消費する（後ろだと Ctrl+Z / Ctrl+S に先取りされる）
        let (sz, ss, z, y, s, o, n, e) = ctx.input_mut(|i| {
            (
                i.consume_shortcut(&cmd_shift(Key::Z)),
                i.consume_shortcut(&cmd_shift(Key::S)),
                i.consume_shortcut(&cmd(Key::Z)),
                i.consume_shortcut(&cmd(Key::Y)),
                i.consume_shortcut(&cmd(Key::S)),
                i.consume_shortcut(&cmd(Key::O)),
                i.consume_shortcut(&cmd(Key::N)),
                i.consume_shortcut(&cmd(Key::E)),
            )
        });
        if s {
            self.save(ctx);
        }
        if ss {
            self.save_as_dialog(ctx);
        }
        if o {
            self.open_dialog();
        }
        if n {
            self.guard(Pending::New);
        }
        if e {
            self.export_dialog(ctx, false);
        }
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        if z {
            self.doc.undo();
        }
        if y || sz {
            self.doc.redo();
        }
        let (c, x, v, d, a, del, esc, zero) = ctx.input_mut(|i| {
            (
                i.consume_shortcut(&cmd(Key::C)),
                i.consume_shortcut(&cmd(Key::X)),
                i.consume_shortcut(&cmd(Key::V)),
                i.consume_shortcut(&cmd(Key::D)),
                i.consume_shortcut(&cmd(Key::A)),
                i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace),
                i.key_pressed(Key::Escape),
                i.consume_shortcut(&cmd(Key::Num0)),
            )
        });
        if c {
            self.clipboard = self.doc.copy_selected();
        }
        if x {
            self.clipboard = self.doc.copy_selected();
            self.doc.delete_selected();
        }
        if v {
            let objs = self.clipboard.clone();
            self.doc.paste(&objs, 2.0 * MM);
        }
        if d {
            self.duplicate();
        }
        if a {
            self.doc.select_all();
        }
        if del {
            self.doc.delete_selected();
        }
        if esc {
            self.doc.selection.clear();
            self.tool = Tool::Select;
        }
        if zero {
            self.view.fit_pending = true;
        }
        let tools = [
            (Key::V, Tool::Select),
            (Key::P, Tool::Picture),
            (Key::T, Tool::Label),
            (Key::D, Tool::Value),
            (Key::R, Tool::Row),
        ];
        for (k, t) in tools {
            if ctx.input(|i| i.modifiers.is_none() && i.key_pressed(k)) {
                self.tool = t;
            }
        }
        // 矢印キーで少しずつ動かす（Shift で 5 mm）
        let (dx, dy, fast) = ctx.input(|i| {
            let mut dx = 0.0;
            let mut dy = 0.0;
            if i.key_pressed(Key::ArrowLeft) {
                dx -= 1.0;
            }
            if i.key_pressed(Key::ArrowRight) {
                dx += 1.0;
            }
            if i.key_pressed(Key::ArrowUp) {
                dy -= 1.0;
            }
            if i.key_pressed(Key::ArrowDown) {
                dy += 1.0;
            }
            (dx, dy, i.modifiers.shift)
        });
        if dx != 0.0 || dy != 0.0 {
            let step = if fast { 5.0 } else { 0.5 } * MM;
            self.doc.move_selected(dx * step, dy * step, Some("nudge"));
        } else if ctx.input(|i| i.keys_down.is_empty()) {
            // 矢印を離したら次の移動は別の「元に戻す」にする
        }
    }
}
