//! Windows のアイコンとファイルバージョン。

#[cfg(windows)]
fn packed_version(version: &str) -> Option<u64> {
    let numeric = version.split(['-', '+']).next()?;
    let mut parts = numeric.split('.').map(|part| part.parse::<u64>().ok());
    let (major, minor, patch) = (parts.next()??, parts.next()??, parts.next()??);
    Some((major << 48) | (minor << 32) | (patch << 16))
}

#[cfg(windows)]
fn main() {
    println!("cargo:rerun-if-changed=Cargo.toml");
    let version = env!("CARGO_PKG_VERSION");
    let packed = packed_version(version).unwrap_or(0);

    let mut res = winres::WindowsResource::new();
    if std::path::Path::new("assets/icon.ico").exists() {
        res.set_icon("assets/icon.ico");
    }
    res.set("FileVersion", version);
    res.set("ProductVersion", version);
    res.set(
        "FileDescription",
        "現場編集長アルバムのカードテンプレート（.TPL）編集アプリ",
    );
    res.set("ProductName", "カードテンプレート工房");
    res.set_version_info(winres::VersionInfo::FILEVERSION, packed);
    res.set_version_info(winres::VersionInfo::PRODUCTVERSION, packed);
    if let Err(e) = res.compile() {
        eprintln!("Failed to compile Windows resource: {e}");
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {}
