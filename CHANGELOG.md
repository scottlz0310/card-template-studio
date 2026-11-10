# Changelog

## [Unreleased]

### Added

- 現場編集長アルバムのカードテンプレート（.TPL）の読み書き。読んで何も変えずに書くとバイト単位で元と一致する。
- 図形・参考図など（種類 4・6〜9）は生のバイト列のまま透過して書き戻す。
- 本体のエクスポート形式（TMPEXP\expinf, TEMP{横}x{縦}_{H|V}\名前.TPL）の一覧表示と書き出し。
- 写真枠・項目名・項目値・項目の行の配置、移動・大きさ変更、吸着ガイド、整列、元に戻す。
- 文字項目の書式（フォント・大きさ・太字・斜体・下線・取り消し線・縦書き・行揃え・行間・文字間・色・枠線）の編集。
- 写真枠の形（四角 / 角丸 / 楕円）・枠線の太さ（なし / 細い / 中 / 太い）・枠線の色を、キャンバスに描き、プロパティパネルで編集できるようにした。実機の差分実験で確定した `b8`（+0xb8）と `bc`（+0xbc）を、`frame_style` と `border_color` に改名した（#25）。描く太さ（mm）と角丸の半径は、実機の見た目からの推定。
- `--screenshot <file.png>` で画面を PNG に保存して終了する確認用の起動方法。
- リポジトリ基盤・規約準拠の補完（`.review-raven/ci-policy.json`、`rust-toolchain.toml`、`LICENSE-MIT` / `LICENSE-APACHE`、`CONTRIBUTING.md`）。
- アプリアイコン案（`assets/icon.png`, `icon.ico`）および `winres` による Windows リソース埋め込み。
- 主要モジュール分割（#6）の前段として、現状の挙動を固定するテストを 17 件から 176 件に拡充。画面操作のテストには `egui_kittest`（dev-dependency）と、ウィンドウを使わない入力シミュレータを使う。

- 実物の TMPEXP の値の分布を集計する調査用テスト `survey_real_samples`（`#[ignore]`、`CARD_TPL_SAMPLES` 指定時に手動実行。#18）。
- 集計テストに、文字項目の `is_value` と項目名・表示文字の関係の列、および `is_value` / `t108` が `-1` の部品を一覧する `list_odd_captions` を追加（#18）。
- 実機の差分実験用に、実物から調べたい値だけを変えた TPL を生成する調査用テスト（`gen_picture_frame_*` / `gen_fmt_*` / `list_fmt_variants` / `fmt_12_13_*` / `dump_template` / `picture_combinations`）を追加（`#[ignore]`、手動実行。#18）。

### Changed

- 新規の文字項目の既定フォントを、本体と同じ「メイリオ」にした（以前は「ＭＳ Ｐゴシック」）。フォントの選択肢に「メイリオ」「游ゴシック」「游明朝」を足した（MS 系は既存のテンプレートのために残す）。メイリオは、代用せず実際のフォントで描く。egui は、ＭＳ ゴシック・ＭＳ 明朝の字形を正しく描けないため、これらは従来どおり画面表示だけ游ゴシック・游明朝で代用する（#28）。
- Codecov の集計対象から、手動実行専用の実験用テスト `src/tpl/experiment.rs` を除外。
- Codecov の集計対象から、手動実行専用の集計テスト `src/tpl/survey.rs` を除外。
- Codecov の集計対象から、OS のネイティブダイアログを呼ぶだけの `src/app/dialogs.rs` を除外（`src/main.rs` と同じ扱い）。
- ファイル・フォルダ選択と上書き確認のダイアログを `Dialogs` trait 越しにし（本体の実装は `rfd` を呼ぶ `NativeDialogs` に集約）、テストでは差し替えられるようにした（#6）。挙動は変わらない。
- `src/app.rs` を責務ごとのサブモジュール（`files` / `shortcuts` / `menu` / `panels` / `modals` / `screenshot`）に分割（#6）。挙動は変わらない。
- 編集面の操作（押す・ドラッグ・離す・クリック）の判断を、egui に依存しない `canvas/interaction.rs` に切り出し、全体表示・ズームの計算を `canvas/view.rs` に移した（#6）。挙動は変わらない。
- `src/canvas.rs` を責務ごとのサブモジュール（`tool` / `view` / `geometry` / `snap` / `state` / `draw` / `show`）に分割（#6）。挙動は変わらない。
- `src/tpl.rs` を責務ごとのサブモジュール（`text` / `io` / `header` / `export` / `format` / `body` / `object` / `shape` / `codec` / `template`）に分割（#6）。挙動・公開 API・.TPL のバイト列は変わらない。

### Fixed

- 新規の写真枠の既定値を、本体が新規に作る写真枠と同じ（四角・枠線なし。`frame_style` / `border_color` = 0）にした。以前は角丸・赤い細枠（`0x11` / `0xff`）で、本体と食い違っていた（#18）。
- 文字項目の `is_value`（+0x10c）が `-1` の部品（実物 14 件。項目に結び付かない固定文字）が、項目値の枠（`〈〉`）として描かれていたのを、項目名と同じく文字として描くようにした。`-1` は読んだまま保持して書き戻す（#22）。

[Unreleased]: https://github.com/scottlz0310/card-template-studio/tree/HEAD
