# Changelog

## [Unreleased]

### Added

- 現場編集長アルバムのカードテンプレート（.TPL）の読み書き。読んで何も変えずに書くとバイト単位で元と一致する。
- 図形・参考図など（種類 4・6〜9）は生のバイト列のまま透過して書き戻す。
- 本体のエクスポート形式（TMPEXP\expinf, TEMP{横}x{縦}_{H|V}\名前.TPL）の一覧表示と書き出し。
- 写真枠・項目名・項目値・項目の行の配置、移動・大きさ変更、吸着ガイド、整列、元に戻す。
- 文字項目の書式（フォント・大きさ・太字・斜体・下線・取り消し線・縦書き・行揃え・行間・文字間・色・枠線）の編集。
- `--screenshot <file.png>` で画面を PNG に保存して終了する確認用の起動方法。
- リポジトリ基盤・規約準拠の補完（`.review-raven/ci-policy.json`、`rust-toolchain.toml`、`LICENSE-MIT` / `LICENSE-APACHE`、`CONTRIBUTING.md`）。
- アプリアイコン案（`assets/icon.png`, `icon.ico`）および `winres` による Windows リソース埋め込み。
- 主要モジュール分割（#6）の前段として、現状の挙動を固定するテストを 17 件から 176 件に拡充。画面操作のテストには `egui_kittest`（dev-dependency）と、ウィンドウを使わない入力シミュレータを使う。

[Unreleased]: https://github.com/scottlz0310/card-template-studio/tree/HEAD
