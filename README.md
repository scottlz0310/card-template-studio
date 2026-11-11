# カードテンプレート工房

現場編集長 CALSMASTER のアルバム作成ツールで使うカードテンプレート（.TPL）を編集する Windows アプリ。
本体の「カード」メニュー →「テンプレートのエクスポート／インポート」で受け渡しする。

## 動作環境

- Windows（x86_64）。日本語の表示には、Windows 標準のフォント（游ゴシック・メイリオ）を使う。フォントは同梱しない。
- 現場編集長 CALSMASTER Ver.25 で、書き出したテンプレートの取り込みと見た目を確かめている。

## 導入

導入の方法は、[Scoop](https://scoop.sh) と、GitHub Releases の ZIP の 2 通り。
インストーラーはなく、設定ファイルやレジストリも使わない（実行ファイルを置いて起動するだけ）。

### Scoop で導入する

```powershell
scoop bucket add scottlz0310 https://github.com/scottlz0310/scoop-bucket
scoop install card-template-studio
```

- 更新は `scoop update card-template-studio`、削除は `scoop uninstall card-template-studio`。
- スタートメニューに「Card Template Studio」のショートカットができる。コマンドラインからは `card-template-studio` で起動できる。
- Scoop は Release の ZIP をダウンロードするときに、`.sha256` でチェックサムを照合する。
- Scoop が取得したファイルには、ブラウザのダウンロードの印が付かないため、下の SmartScreen の警告は出にくい。

### ZIP から導入する

[GitHub Releases](https://github.com/scottlz0310/card-template-studio/releases) の ZIP を、展開して使う。消すときは、フォルダごと削除する。

1. 最新の Release から、次の 2 つをダウンロードする。
   - `card-template-studio-x86_64-pc-windows-msvc.zip`
   - `card-template-studio-x86_64-pc-windows-msvc.zip.sha256`
2. ダウンロードしたフォルダで PowerShell を開き、ZIP のチェックサムを照合する。

   ```powershell
   $expected = (Get-Content card-template-studio-x86_64-pc-windows-msvc.zip.sha256).Split(' ')[0]
   $actual = (Get-FileHash -Algorithm SHA256 card-template-studio-x86_64-pc-windows-msvc.zip).Hash
   if ($expected -eq $actual) { 'OK: 一致' } else { 'NG: 一致しない（使わずに削除する）' }
   ```

   `-eq` は大文字小文字を区別しないので、`.sha256` の小文字と `Get-FileHash` の大文字は、そのまま比べてよい。
3. ZIP を好きな場所に展開し、`card-template-studio.exe` を起動する。

#### SmartScreen の警告が出たとき

exe はコード署名をしていない。ブラウザでダウンロードした ZIP の初回実行で、「Windows によって PC が保護されました」と出ることがある。

- 展開する前に、ZIP のファイルを右クリック →「プロパティ」→「許可する」にチェックを入れて「OK」を押す。
- または、警告の「詳細情報」→「実行」を押す。

照合したチェックサムが一致していれば、Release の成果物と同じ内容を確かめられている。

### ソースから導入する

Rust（バージョンは `rust-toolchain.toml`）を用意した上で、次を実行する。`%USERPROFILE%\.cargo\bin` にビルドした exe が入る。

```
cargo install --git https://github.com/scottlz0310/card-template-studio --locked
```

## 使い方

1. 本体で「テンプレートのエクスポート」を行い、TMPEXP フォルダを作る。
2. このアプリの「ファイル」→「エクスポートしたフォルダ（TMPEXP）を開く」で一覧を出し、編集する。
3. 「本体用に書き出す」で TMPEXP フォルダを作り、本体の「テンプレートのインポート」で取り込む。

ファイルを開いて起動することもできる（TMPEXP フォルダも可）。

```
card-template-studio.exe 名前.TPL
```

## 開発

ビルドと起動:

```
cargo build --release
target\release\card-template-studio.exe
```

開発中は `cargo build` の `target\debug\card-template-studio.exe` でも同じように動く。

### 確認

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
