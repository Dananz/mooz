//! Settings persisted as JSON in %APPDATA%\Mooz\config.json. Every process
//! (tray, hook thread, settings window) re-reads the file when its mtime
//! changes, the Windows stand-in for the macOS app reading UserDefaults live.
//! Missing keys fall back to defaults, so old files keep working.

use super::modifiers::Modifiers;
use super::shortcut::Shortcut;
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
    time::SystemTime,
};

pub const SENSITIVITY_MIN: f64 = 0.1;
pub const SENSITIVITY_MAX: f64 = 3.0;
pub const SENSITIVITY_DEFAULT: f64 = 0.5;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InputSource {
    #[default]
    MouseDrag,
    ScrollWheel,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ListMode {
    #[default]
    Blocklist,
    Allowlist,
}

/// How the zoom reaches apps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ZoomMethod {
    /// A two-finger pinch on a virtual precision touchpad (Windows 11): the
    /// same smooth, cursor-anchored zoom a real touchpad gives.
    #[default]
    Pinch,
    /// Ctrl + mouse wheel: works everywhere, but apps do their step zoom.
    Wheel,
}

/// How Ctrl + wheel zoom ticks are sent (Wheel method only).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ZoomStyle {
    /// Whole wheel notches (120). Every app handles these correctly.
    #[default]
    Steps,
    /// Fine-grained wheel deltas. Smoother where an app honors the delta size,
    /// but apps that zoom one step per wheel message will over-zoom.
    Smooth,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppEntry {
    /// Executable file name, e.g. "chrome.exe". Matched case-insensitively.
    pub exe: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    pub enabled: bool,
    pub modifiers: Modifiers,
    /// Empty in legacy files: use the original modifiers field.
    pub shortcuts: Vec<Shortcut>,
    pub input_source: InputSource,
    pub sensitivity: f64,
    pub anchor_cursor: bool,
    pub zoom_method: ZoomMethod,
    pub zoom_style: ZoomStyle,
    pub list_mode: ListMode,
    pub apps: Vec<AppEntry>,
    /// Set once the first-run tray notification has been shown.
    pub onboarded: bool,
    /// Hidden: write a diagnostic log (Windows: %TEMP%\mooz-debug.log).
    pub debug: bool,
    /// Background update checks only. Downloads and restarts require a click.
    pub check_updates: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            modifiers: Modifiers::SHIFT,
            shortcuts: Vec::new(),
            input_source: InputSource::MouseDrag,
            sensitivity: SENSITIVITY_DEFAULT,
            anchor_cursor: true,
            zoom_method: ZoomMethod::Pinch,
            zoom_style: ZoomStyle::Steps,
            list_mode: ListMode::Blocklist,
            apps: Vec::new(),
            onboarded: false,
            debug: false,
            check_updates: true,
        }
    }
}

impl Config {
    /// Clamps values a hand-edited file could put out of range.
    pub fn sanitized(mut self) -> Self {
        self.modifiers = self.modifiers.masked();
        let mut unique = Vec::new();
        for mut shortcut in self.shortcuts {
            shortcut.modifiers = shortcut.modifiers.masked();
            if shortcut.valid() && !unique.contains(&shortcut) { unique.push(shortcut); }
        }
        self.shortcuts = unique;
        self.sensitivity = if self.sensitivity.is_finite() {
            self.sensitivity.clamp(SENSITIVITY_MIN, SENSITIVITY_MAX)
        } else {
            SENSITIVITY_DEFAULT
        };
        self
    }

    pub fn effective_shortcuts(&self) -> Vec<Shortcut> {
        if self.shortcuts.is_empty() { vec![Shortcut { modifiers: self.modifiers, key: None }] }
        else { self.shortcuts.clone() }
    }

    pub fn set_shortcuts(&mut self, shortcuts: Vec<Shortcut>) {
        if let Some(first) = shortcuts.first() {
            self.modifiers = first.modifiers;
            self.shortcuts = shortcuts;
        }
    }

    pub fn shortcut_label(&self) -> String {
        self.effective_shortcuts().iter().map(|s| s.label()).collect::<Vec<_>>().join(" or ")
    }

    pub fn from_json(text: &str) -> Option<Self> {
        serde_json::from_str::<Self>(text).ok().map(Self::sanitized)
    }

    /// Reads the file, falling back to defaults when it is missing or invalid.
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path)
            .ok()
            .and_then(|t| Self::from_json(&t))
            .unwrap_or_default()
    }

    /// Writes through a temp file and rename so a concurrent reader never sees
    /// a half-written file.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        let text = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        fs::write(&tmp, text)?;
        fs::rename(&tmp, path)
    }
}

/// %APPDATA%\Mooz\config.json on Windows. Elsewhere (the settings preview on
/// macOS) it lives under ~/.config/mooz-windows-preview. MOOZ_CONFIG overrides both.
pub fn config_path() -> PathBuf {
    if let Some(p) = std::env::var_os("MOOZ_CONFIG") {
        return PathBuf::from(p);
    }
    if cfg!(windows) {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            return PathBuf::from(appdata).join("Mooz").join("config.json");
        }
    }
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
    PathBuf::from(home.unwrap_or_default())
        .join(".config")
        .join("mooz-windows-preview")
        .join("config.json")
}

pub fn modified_time(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Caches the config and reloads it only when the file's mtime changes.
pub struct ConfigWatcher {
    path: PathBuf,
    mtime: Option<SystemTime>,
    pub config: Config,
}

impl ConfigWatcher {
    pub fn new(path: PathBuf) -> Self {
        let mtime = modified_time(&path);
        let config = Config::load(&path);
        Self { path, mtime, config }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns true when the config was reloaded.
    pub fn refresh(&mut self) -> bool {
        let mtime = modified_time(&self.path);
        if mtime == self.mtime {
            return false;
        }
        self.mtime = mtime;
        self.config = Config::load(&self.path);
        true
    }

    /// Saves and records the new mtime so our own write isn't seen as external.
    pub fn save(&mut self) -> io::Result<()> {
        self.config.save(&self.path)?;
        self.mtime = modified_time(&self.path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_macos_app() {
        let c = Config::default();
        assert!(c.enabled);
        assert_eq!(c.modifiers, Modifiers::SHIFT);
        assert_eq!(c.input_source, InputSource::MouseDrag);
        assert_eq!(c.sensitivity, 0.5);
        assert!(c.anchor_cursor);
        assert_eq!(c.list_mode, ListMode::Blocklist);
        assert_eq!(c.zoom_method, ZoomMethod::Pinch);
    }

    #[test]
    fn missing_keys_fall_back_to_defaults() {
        let c = Config::from_json(r#"{"inputSource":"scrollWheel"}"#).unwrap();
        assert_eq!(c.input_source, InputSource::ScrollWheel);
        assert!(c.enabled);
        assert!(c.anchor_cursor);
        assert_eq!(c.modifiers, Modifiers::SHIFT);
    }

    #[test]
    fn out_of_range_values_are_clamped() {
        let c = Config::from_json(r#"{"sensitivity":99,"modifiers":255}"#).unwrap();
        assert_eq!(c.sensitivity, SENSITIVITY_MAX);
        assert_eq!(c.modifiers, Modifiers::ALL);
    }

    #[test]
    fn legacy_shortcut_migrates_without_losing_modifiers() {
        let c=Config::from_json(r#"{"modifiers":3}"#).unwrap();
        assert_eq!(c.effective_shortcuts(),vec![Shortcut{modifiers:Modifiers::CTRL|Modifiers::ALT,key:None}]);
        let mut c=c;
        let shortcuts=vec![Shortcut{modifiers:Modifiers::CTRL,key:Some(0x4B)},Shortcut{modifiers:Modifiers::SHIFT,key:None}];
        c.set_shortcuts(shortcuts.clone());
        assert_eq!(c.modifiers,Modifiers::CTRL);
        let json=serde_json::to_string(&c).unwrap();
        assert_eq!(Config::from_json(&json).unwrap().effective_shortcuts(),shortcuts);
    }

    #[test]
    fn invalid_and_duplicate_shortcuts_are_removed() {
        let c=Config::from_json(r#"{"modifiers":4,"shortcuts":[{"modifiers":0},{"modifiers":255,"key":999},{"modifiers":4},{"modifiers":4}]}"#).unwrap();
        assert_eq!(c.effective_shortcuts(),vec![Shortcut{modifiers:Modifiers::SHIFT,key:None}]);
        assert_eq!(Config::from_json(r#"{"shortcuts":[]}"#).unwrap().shortcut_label(),"Shift");
    }

    #[test]
    fn invalid_json_loads_defaults() {
        let dir = std::env::temp_dir().join(format!("mooz-test-{}", std::process::id()));
        let path = dir.join("bad.json");
        fs::create_dir_all(&dir).unwrap();
        fs::write(&path, "{not json").unwrap();
        assert_eq!(Config::load(&path), Config::default());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn save_round_trips_and_watcher_sees_external_change() {
        let dir = std::env::temp_dir().join(format!("mooz-test-rt-{}", std::process::id()));
        let path = dir.join("config.json");
        let mut c = Config { sensitivity: 1.7, ..Config::default() };
        c.apps.push(AppEntry { exe: "chrome.exe".into(), name: "Chrome".into() });
        c.save(&path).unwrap();
        assert_eq!(Config::load(&path), c);

        let mut watcher = ConfigWatcher::new(path.clone());
        assert!(!watcher.refresh());
        // Sleep past coarse mtime resolution, then write from "another process".
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let mut other = c.clone();
        other.enabled = false;
        other.save(&path).unwrap();
        assert!(watcher.refresh());
        assert!(!watcher.config.enabled);
        fs::remove_dir_all(&dir).ok();
    }
}
