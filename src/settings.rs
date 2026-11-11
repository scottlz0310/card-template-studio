//! 利用者の操作の設定（最後に使ったフォルダ、表示の設定）の保存と読み込み。
//!
//! 保存先は `%APPDATA%\card-template-studio\settings.json`。テンプレートの内容は保存しない
//! （業務データを設定に残さない）。ウィンドウの大きさ・位置は eframe の永続化（同じフォルダの
//! `data\app.ron`）が受け持つので、ここでは扱わない。
//!
//! アプリ本体は `SettingsStore` だけを知り、ファイルには触れない。テストでは別の実装に差し替える。

use crate::canvas::{GRID_MM_RANGE, View};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// `%APPDATA%` の下に作るフォルダ名。eframe の永続化（`run_native` の名前）と同じにして、1 か所にまとめる
pub const APP_DIR: &str = "card-template-studio";
const FILE_NAME: &str = "settings.json";

/// 保存する設定。足りない項目は既定値、知らない項目は無視する（版が変わっても読める）
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub folders: Folders,
    pub view: ViewSettings,
}

/// ファイルダイアログの開始位置に使う、最後に使ったフォルダ
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Folders {
    /// 最後に開いた TMPEXP フォルダ
    pub library: Option<PathBuf>,
    /// 最後にエクスポートした書き出し先
    pub export_dest: Option<PathBuf>,
}

/// 表示の設定（拡大率とスクロール位置は、カードに合わせて決まるので保存しない）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewSettings {
    pub show_grid: bool,
    pub grid_mm: f32,
    pub snap_grid: bool,
    pub snap_objects: bool,
    pub show_used: bool,
    pub sample_values: bool,
}

impl Default for ViewSettings {
    fn default() -> Self {
        Self::of(&View::default())
    }
}

impl ViewSettings {
    pub fn of(view: &View) -> Self {
        Self {
            show_grid: view.show_grid,
            grid_mm: view.grid_mm,
            snap_grid: view.snap_grid,
            snap_objects: view.snap_objects,
            show_used: view.show_used,
            sample_values: view.sample_values,
        }
    }

    pub fn apply_to(&self, view: &mut View) {
        view.show_grid = self.show_grid;
        view.grid_mm = self.grid_mm;
        view.snap_grid = self.snap_grid;
        view.snap_objects = self.snap_objects;
        view.show_used = self.show_used;
        view.sample_values = self.sample_values;
    }
}

impl Settings {
    /// 手で書き換えられた値が、画面で選べる範囲の外なら、読み込みの失敗として扱う
    fn validate(&self) -> Result<(), String> {
        let grid = self.view.grid_mm;
        if !GRID_MM_RANGE.contains(&grid) {
            return Err(format!(
                "方眼の間隔（grid_mm = {grid}）は {} 〜 {} mm の範囲で指定してください",
                GRID_MM_RANGE.start(),
                GRID_MM_RANGE.end()
            ));
        }
        Ok(())
    }
}

/// 設定の保存先。エラーは、原因が分かる文にして返す
pub trait SettingsStore {
    /// 設定を読む。まだ保存されていない（初回の起動）ときは、既定の設定を返す
    fn load(&self) -> Result<Settings, String>;
    fn save(&self, settings: &Settings) -> Result<(), String>;
}

/// `settings.json`
pub struct FileStore {
    /// 場所が決まらなかったときは、その理由
    path: Result<PathBuf, String>,
}

impl FileStore {
    /// `%APPDATA%\card-template-studio\settings.json`。
    /// eframe の保存先（`...\card-template-studio\data`）の 1 つ上にすることで、eframe と必ず同じフォルダになる
    /// （eframe は環境変数ではなく Windows の既知フォルダ API で、`%APPDATA%` を決める）。
    pub fn in_app_data() -> Self {
        Self::from_app_dir(
            eframe::storage_dir(APP_DIR).and_then(|data| data.parent().map(Path::to_path_buf)),
        )
    }

    fn from_app_dir(app_dir: Option<PathBuf>) -> Self {
        Self {
            path: app_dir
                .filter(|p| !p.as_os_str().is_empty())
                .map(|p| p.join(FILE_NAME))
                .ok_or_else(|| {
                    "設定の保存先（%APPDATA%）が分からないため、設定は保存されません".into()
                }),
        }
    }

    #[cfg(test)]
    pub(crate) fn at(path: PathBuf) -> Self {
        Self { path: Ok(path) }
    }

    fn path(&self) -> Result<&Path, String> {
        self.path.as_deref().map_err(Clone::clone)
    }
}

impl SettingsStore for FileStore {
    fn load(&self) -> Result<Settings, String> {
        let path = self.path()?;
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Settings::default()),
            Err(e) => return Err(format!("{} を読めません: {e}", path.display())),
        };
        let invalid = |e: &dyn std::fmt::Display| {
            format!(
                "{} の内容が不正です: {e}\n次に設定が変わると、このファイルは新しい内容で上書きされます。",
                path.display()
            )
        };
        let settings: Settings = serde_json::from_slice(&bytes).map_err(|e| invalid(&e))?;
        settings.validate().map_err(|e| invalid(&e))?;
        Ok(settings)
    }

    fn save(&self, settings: &Settings) -> Result<(), String> {
        let path = self.path()?;
        let json = serde_json::to_string_pretty(settings)
            .map_err(|e| format!("設定を JSON にできません: {e}"))?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("{} を作れません: {e}", dir.display()))?;
        }
        // 書き込み中に止まっても、元のファイルを壊さないよう、一時ファイルに書いてから置き換える。
        // 複数のアプリを同時に起動していても、書き込みが交錯しないよう、保存ごとに固有の名前で
        // 排他的に作り（すでにあればエラー）、置き換えと削除は、自分が作った一時ファイルだけに行う
        let tmp = temp_path(path);
        write_new(&tmp, json.as_bytes())
            .map_err(|e| format!("{} に書けません: {e}", tmp.display()))?;
        std::fs::rename(&tmp, path).map_err(|e| {
            // 置き換えに失敗した一時ファイルを残さない。ここでの失敗は、本来のエラーを隠さないよう無視する
            let _ = std::fs::remove_file(&tmp);
            format!("{} に書けません: {e}", path.display())
        })
    }
}

/// 保存ごとに固有の一時ファイル名（`settings.json.<プロセス ID>.<連番>.tmp`）。
/// プロセス ID が別の起動と、連番が同じプロセスの別の保存と、名前が重ならないようにする
fn temp_path(path: &Path) -> PathBuf {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(
        ".{}.{}.tmp",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    path.with_file_name(name)
}

/// 新しいファイルを作って書く。同じ名前がすでにあれば、上書きせずにエラーにする。
/// 書き込みに失敗したら、作ったファイルを消す（失敗の原因を隠さないよう、消す失敗は無視する）
fn write_new(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    if written.is_err() {
        drop(file);
        let _ = std::fs::remove_file(path);
    }
    written
}

/// テスト用: メモリに保存し、呼ばれ方を記録する。app とテストで同じ状態を共有できる
#[cfg(test)]
pub mod memory {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Default)]
    pub struct State {
        pub stored: Settings,
        pub load_error: Option<String>,
        pub save_error: Option<String>,
        /// 保存を試みた内容（失敗したものも含む）
        pub saves: Vec<Settings>,
    }

    #[derive(Clone, Default)]
    pub struct MemoryStore(pub Rc<RefCell<State>>);

    impl SettingsStore for MemoryStore {
        fn load(&self) -> Result<Settings, String> {
            let s = self.0.borrow();
            match &s.load_error {
                Some(e) => Err(e.clone()),
                None => Ok(s.stored.clone()),
            }
        }

        fn save(&self, settings: &Settings) -> Result<(), String> {
            let mut s = self.0.borrow_mut();
            s.saves.push(settings.clone());
            match &s.save_error {
                Some(e) => Err(e.clone()),
                None => {
                    s.stored = settings.clone();
                    Ok(())
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    fn sample() -> Settings {
        Settings {
            folders: Folders {
                library: Some(PathBuf::from(r"C:\Template\TMPEXP")),
                export_dest: Some(PathBuf::from(r"D:\出力")),
            },
            view: ViewSettings {
                show_grid: false,
                grid_mm: 2.5,
                snap_grid: false,
                snap_objects: true,
                show_used: false,
                sample_values: true,
            },
        }
    }

    #[test]
    fn default_view_settings_match_the_view_defaults() {
        let mut view = View {
            show_grid: false,
            grid_mm: 7.0,
            ..View::default()
        };
        ViewSettings::default().apply_to(&mut view);
        let d = View::default();
        assert_eq!(
            (
                view.show_grid,
                view.grid_mm,
                view.snap_grid,
                view.snap_objects
            ),
            (d.show_grid, d.grid_mm, d.snap_grid, d.snap_objects)
        );
        assert_eq!(
            (view.show_used, view.sample_values),
            (d.show_used, d.sample_values)
        );
    }

    #[test]
    fn view_settings_round_trip_through_the_view() {
        let mut view = View::default();
        sample().view.apply_to(&mut view);
        assert_eq!(ViewSettings::of(&view), sample().view);
    }

    #[test]
    fn view_settings_never_carry_zoom_or_pan() {
        // 拡大率とスクロール位置は、カードに合わせて決まる。保存する項目に入っていないことを、JSON のキーで確かめる
        let json = serde_json::to_value(sample()).unwrap();
        let keys: Vec<&str> = json["view"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        for k in ["zoom", "pan", "fit_pending"] {
            assert!(!keys.contains(&k), "{k} は保存しない");
        }
    }

    #[test]
    fn save_then_load_returns_the_same_settings() {
        let dir = TempDir::new("settings-roundtrip");
        let store = FileStore::at(dir.path().join("nested").join("settings.json"));
        store.save(&sample()).expect("保存");
        assert_eq!(store.load().expect("読み込み"), sample());
    }

    #[test]
    fn save_replaces_the_file_and_leaves_no_temporary_file() {
        let dir = TempDir::new("settings-atomic");
        let path = dir.path().join("settings.json");
        let store = FileStore::at(path.clone());
        store.save(&Settings::default()).expect("1 回目");
        store.save(&sample()).expect("2 回目");
        assert_eq!(store.load().unwrap(), sample());
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["settings.json"]);
    }

    #[test]
    fn missing_file_loads_defaults_without_error() {
        let dir = TempDir::new("settings-missing");
        let store = FileStore::at(dir.path().join("settings.json"));
        assert_eq!(store.load().expect("初回の起動は正常"), Settings::default());
    }

    #[test]
    fn loading_tolerates_missing_and_unknown_fields() {
        let cases = [
            ("{}", Settings::default()),
            (
                r#"{"view": {"show_grid": false}}"#,
                Settings {
                    view: ViewSettings {
                        show_grid: false,
                        ..ViewSettings::default()
                    },
                    ..Settings::default()
                },
            ),
            (
                r#"{"folders": {"library": "C:\\a"}, "future_field": 1, "view": {"zoom": 3}}"#,
                Settings {
                    folders: Folders {
                        library: Some(PathBuf::from(r"C:\a")),
                        export_dest: None,
                    },
                    ..Settings::default()
                },
            ),
        ];
        for (json, expected) in cases {
            let dir = TempDir::new("settings-lenient");
            let path = dir.path().join("settings.json");
            std::fs::write(&path, json).unwrap();
            assert_eq!(FileStore::at(path).load().expect(json), expected, "{json}");
        }
    }

    #[test]
    fn invalid_files_are_reported_with_the_path_and_the_reason() {
        let cases = [
            ("not json", "settings.json の内容が不正です"),
            ("", "settings.json の内容が不正です"),
            (
                r#"{"view": {"show_grid": "yes"}}"#,
                "settings.json の内容が不正です",
            ),
            (r#"{"view": {"grid_mm": 0}}"#, "grid_mm"),
            (r#"{"view": {"grid_mm": 99}}"#, "grid_mm"),
            (r#"{"view": {"grid_mm": -1}}"#, "grid_mm"),
        ];
        for (json, expected) in cases {
            let dir = TempDir::new("settings-invalid");
            let path = dir.path().join("settings.json");
            std::fs::write(&path, json).unwrap();
            let err = FileStore::at(path).load().expect_err(json);
            assert!(err.contains(expected), "{json:?} → {err}");
            assert!(err.contains("settings.json"), "{json:?} → {err}");
        }
    }

    #[test]
    fn unreadable_path_is_reported_not_treated_as_missing() {
        // ファイルのはずの場所がフォルダ: 「無い」ではなく、読めないという失敗にする
        let dir = TempDir::new("settings-unreadable");
        let path = dir.path().join("settings.json");
        std::fs::create_dir(&path).unwrap();
        let err = FileStore::at(path).load().expect_err("読めない");
        assert!(err.contains("を読めません"), "{err}");
    }

    #[test]
    fn unwritable_destinations_are_reported_with_the_path() {
        // 親がファイルで、フォルダを作れない／置き換え先がフォルダで、書けない
        let dir = TempDir::new("settings-unwritable");
        let file = dir.path().join("file");
        std::fs::write(&file, "x").unwrap();
        let err = FileStore::at(file.join("settings.json"))
            .save(&Settings::default())
            .expect_err("親がファイル");
        assert!(
            err.contains("を作れません") || err.contains("に書けません"),
            "{err}"
        );

        let blocked = dir.path().join("blocked");
        std::fs::create_dir_all(blocked.join("settings.json")).unwrap();
        let err = FileStore::at(blocked.join("settings.json"))
            .save(&Settings::default())
            .expect_err("置き換え先がフォルダ");
        assert!(err.contains("に書けません"), "{err}");
        assert_eq!(
            entries(&blocked),
            ["settings.json"],
            "失敗した一時ファイルを残さない"
        );
    }

    /// フォルダの中のファイル名（並べ替え済み）
    fn entries(dir: &Path) -> Vec<String> {
        let mut names: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn every_save_uses_its_own_temporary_file_name() {
        let path = Path::new(r"C:\data\settings.json");
        let names: Vec<_> = (0..100).map(|_| temp_path(path)).collect();
        let unique: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(unique.len(), names.len(), "名前が重ならない");
        for n in &names {
            assert_eq!(n.parent(), path.parent(), "置き換え先と同じフォルダ");
            let file = n.file_name().unwrap().to_string_lossy();
            assert!(
                file.starts_with("settings.json.") && file.ends_with(".tmp"),
                "{file}"
            );
            assert!(file.contains(&std::process::id().to_string()), "{file}");
        }
    }

    #[test]
    fn write_new_refuses_to_overwrite_an_existing_file() {
        let dir = TempDir::new("settings-write-new");
        let path = dir.path().join("x.tmp");
        std::fs::write(&path, "他の起動の書き込み途中").unwrap();
        let err = write_new(&path, b"new").expect_err("すでにある");
        assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "他の起動の書き込み途中",
            "中身に触れない"
        );
    }

    #[test]
    fn saving_never_touches_another_instances_temporary_file() {
        // 別の起動が書き込み中の一時ファイル（固定名だった頃の名前を含む）を、読まず・書かず・消さない
        let dir = TempDir::new("settings-foreign-tmp");
        let path = dir.path().join("settings.json");
        let foreign = ["settings.json.tmp", "settings.json.99999.0.tmp"];
        for name in foreign {
            std::fs::write(dir.path().join(name), "書き込み途中の長い長い内容").unwrap();
        }
        let store = FileStore::at(path);
        store.save(&sample()).expect("保存");
        assert_eq!(store.load().unwrap(), sample());
        for name in foreign {
            assert_eq!(
                std::fs::read_to_string(dir.path().join(name)).unwrap(),
                "書き込み途中の長い長い内容",
                "{name}"
            );
        }
        assert_eq!(
            entries(dir.path()),
            [
                "settings.json",
                "settings.json.99999.0.tmp",
                "settings.json.tmp"
            ]
        );
    }

    #[test]
    fn concurrent_saves_of_different_lengths_leave_one_complete_json() {
        // 複数起動の再現: 長さの違う設定を、同時に何度も保存する。壊れた JSON や、一時ファイルが残らないこと
        let dir = TempDir::new("settings-concurrent");
        let path = dir.path().join("settings.json");
        let long = Settings {
            folders: Folders {
                library: Some(PathBuf::from(format!(
                    r"C:\{}",
                    "長いフォルダ名".repeat(300)
                ))),
                export_dest: Some(PathBuf::from(format!(r"D:\{}", "出力".repeat(300)))),
            },
            ..sample()
        };
        let short = Settings::default();
        let errors = std::sync::Mutex::new(Vec::<String>::new());
        let saved = AtomicU64::new(0);
        std::thread::scope(|s| {
            for t in 0..8 {
                let (path, long, short, errors, saved) = (&path, &long, &short, &errors, &saved);
                s.spawn(move || {
                    let store = FileStore::at(path.clone());
                    for i in 0..30 {
                        let value = if (t + i) % 2 == 0 { long } else { short };
                        match store.save(value) {
                            Ok(()) => {
                                saved.fetch_add(1, Ordering::Relaxed);
                            }
                            Err(e) => errors.lock().unwrap().push(e),
                        }
                    }
                });
            }
        });
        assert!(
            saved.load(Ordering::Relaxed) > 0,
            "少なくとも一部は成功する"
        );
        // 同時に置き換えると、OS が一部を拒否することがある。そのときは、壊さずにエラーで返る
        for e in errors.lock().unwrap().iter() {
            assert!(e.contains("に書けません"), "{e}");
        }
        let loaded = FileStore::at(path).load().expect("壊れていない JSON");
        assert!(loaded == long || loaded == short, "どちらかの完全な内容");
        assert_eq!(
            entries(dir.path()),
            ["settings.json"],
            "一時ファイルが残らない"
        );
    }

    #[test]
    fn app_data_location_is_next_to_the_eframe_storage_or_reported() {
        let ok = FileStore::from_app_dir(Some(PathBuf::from(
            r"C:\Users\u\AppData\Roaming\card-template-studio",
        )));
        assert_eq!(
            ok.path().unwrap(),
            Path::new(r"C:\Users\u\AppData\Roaming\card-template-studio\settings.json")
        );

        // eframe の保存先（...\card-template-studio\data）の 1 つ上と一致する。実際の場所は書かない（環境に依存するため）
        let real = FileStore::in_app_data();
        let data = eframe::storage_dir(APP_DIR).expect("Windows では決まる");
        assert_eq!(
            real.path().unwrap(),
            data.parent().unwrap().join("settings.json")
        );
        assert!(
            real.path()
                .unwrap()
                .ends_with("card-template-studio/settings.json")
        );

        for missing in [None, Some(PathBuf::new())] {
            let store = FileStore::from_app_dir(missing);
            let load = store.load().expect_err("APPDATA が無い");
            let save = store
                .save(&Settings::default())
                .expect_err("APPDATA が無い");
            assert!(load.contains("APPDATA"), "{load}");
            assert_eq!(load, save);
        }
    }
}
