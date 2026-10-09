//! Single-sources the version from the repo-root VERSION file (like the macOS
//! app and the website) and, for Windows targets, embeds the exe icon, version
//! info and the application manifest.

use std::{env, fs, path::PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let version_file = manifest_dir.join("../VERSION");
    println!("cargo:rerun-if-changed={}", version_file.display());
    println!("cargo:rerun-if-changed=assets/mooz.ico");
    println!("cargo:rerun-if-changed=assets/mooz.manifest");

    let version = fs::read_to_string(&version_file)
        .map(|v| v.trim().to_string())
        .unwrap_or_else(|_| "0.0.0".into());
    println!("cargo:rustc-env=MOOZ_VERSION={version}");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let mut parts: Vec<u16> = version.split('.').map(|p| p.parse().unwrap_or(0)).collect();
    parts.resize(4, 0);
    let commas = parts.iter().map(u16::to_string).collect::<Vec<_>>().join(",");
    let icon = manifest_dir.join("assets/mooz.ico");
    let app_manifest = manifest_dir.join("assets/mooz.manifest");
    let esc = |p: &PathBuf| p.display().to_string().replace('\\', "\\\\");

    let rc = format!(
        r#"#pragma code_page(65001)
1 ICON "{icon}"
1 24 "{manifest}"
1 VERSIONINFO
FILEVERSION {commas}
PRODUCTVERSION {commas}
FILEOS 0x40004
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904b0"
    BEGIN
      VALUE "CompanyName", "Tomer Danan"
      VALUE "FileDescription", "Mooz"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "Mooz"
      VALUE "LegalCopyright", "MIT License, Tomer Danan"
      VALUE "OriginalFilename", "Mooz.exe"
      VALUE "ProductName", "Mooz"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#,
        icon = esc(&icon),
        manifest = esc(&app_manifest),
    );
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("mooz.rc");
    fs::write(&out, rc).unwrap();
    embed_resource::compile(&out, embed_resource::NONE)
        .manifest_required()
        .unwrap();
}
