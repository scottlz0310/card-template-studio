# card-template-studio への貢献

貢献ありがとうございます。変更は小さく、目的を一つに絞ってください。

## 開発環境

- Rust stable（バージョンは `rust-toolchain.toml` を参照）
- Bun（バージョンは `.bun-version` を参照）

依存関係をインストールし、Lefthook の Git フックを設定します。

```sh
bun install
```

## 変更の確認

変更に関係するチェックを実行してから PR を開いてください。

```sh
# Markdown、JSON、YAML などの lint と format
bun run check

# Rust の format、lint、テスト
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

CI でも `bun run check`、Rust の format と Clippy、`cargo llvm-cov` による Rust のテストとカバレッジを確認します。
Lefthook はコミット前に変更されたファイルの Biome、Rust ファイルの format と Clippy を実行し、push 前に `cargo test` を実行します。

## 構成の概要

- `src/tpl.rs`: .TPL ファイルのバイナリ解析・デシリアライズ・シリアライズ
- `src/doc.rs`: テンプレートの編集状態、オブジェクト管理、undo/redo
- `src/layout.rs`: カード寸法、写真枠・項目のグリッドレイアウト計算
- `src/canvas.rs`: egui によるプレビュー描画、ドラッグ＆ドロップ、吸着
- `src/props.rs`: 選択中オブジェクトのプロパティ設定パネル
- `src/app.rs`: メインウィンドウ、メニュー、ファイル入出力
- `docs/MECHANICS.md`: リバースエンジニアリングで判明したカードテンプレート形式の仕様書
