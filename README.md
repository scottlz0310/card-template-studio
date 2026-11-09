# カードテンプレート工房

現場編集長 CALSMASTER のアルバム作成ツールで使うカードテンプレート（.TPL）を編集する Windows アプリ。
本体の「カード」メニュー →「テンプレートのエクスポート／インポート」で受け渡しする。

## 使い方

1. 本体で「テンプレートのエクスポート」を行い、TMPEXP フォルダを作る。
2. このアプリの「ファイル」→「エクスポートしたフォルダ（TMPEXP）を開く」で一覧を出し、編集する。
3. 「本体用に書き出す」で TMPEXP フォルダを作り、本体の「テンプレートのインポート」で取り込む。

## ビルドと起動

```
cargo build --release
target\release\card-template-studio.exe            # 通常の起動
target\release\card-template-studio.exe 名前.TPL   # ファイルを開いて起動（TMPEXP フォルダも可）
```

開発中は `cargo build` の `target\debug\card-template-studio.exe` でも同じように動く。

## 確認

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

実物のテンプレートでの往復一致（読んで書いたら同じバイト列）を確かめるときは、
環境変数 `CARD_TPL_SAMPLES` に TMPEXP フォルダを指定して `cargo test` を実行する。
テンプレートの実物はリポジトリに入れない。

## 形式

逆コンパイルで確かめた形式と、未確定の値は [MECHANICS.md](docs/MECHANICS.md) にまとめている。
未確定のバイトは名前を付けずにそのまま保持し、画面からは変更しない。
