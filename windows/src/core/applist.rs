//! Per-app blocklist / allowlist, keyed by executable name instead of the
//! macOS bundle ID.

use super::config::{AppEntry, ListMode};

/// Lowercased file name of an executable path: `C:\X\Chrome.EXE` -> `chrome.exe`.
pub fn exe_name(path: &str) -> String {
    path.rsplit(['\\', '/']).next().unwrap_or(path).to_lowercase()
}

/// Whether Mooz should act while `foreground_exe` is in front. An unknown
/// foreground app always intercepts, matching the macOS app.
pub fn should_intercept(mode: ListMode, apps: &[AppEntry], foreground_exe: Option<&str>) -> bool {
    let Some(exe) = foreground_exe else { return true };
    let in_list = apps.iter().any(|a| a.exe.eq_ignore_ascii_case(exe));
    match mode {
        ListMode::Blocklist => !in_list,
        ListMode::Allowlist => in_list,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apps() -> Vec<AppEntry> {
        vec![AppEntry { exe: "Chrome.exe".into(), name: "Chrome".into() }]
    }

    #[test]
    fn blocklist_skips_listed_apps() {
        assert!(!should_intercept(ListMode::Blocklist, &apps(), Some("chrome.exe")));
        assert!(should_intercept(ListMode::Blocklist, &apps(), Some("firefox.exe")));
    }

    #[test]
    fn allowlist_only_listed_apps() {
        assert!(should_intercept(ListMode::Allowlist, &apps(), Some("CHROME.EXE")));
        assert!(!should_intercept(ListMode::Allowlist, &apps(), Some("firefox.exe")));
    }

    #[test]
    fn unknown_foreground_intercepts() {
        assert!(should_intercept(ListMode::Allowlist, &apps(), None));
    }

    #[test]
    fn exe_name_strips_path_and_case() {
        assert_eq!(exe_name(r"C:\Program Files\Google\Chrome\Application\Chrome.EXE"), "chrome.exe");
        assert_eq!(exe_name("firefox.exe"), "firefox.exe");
    }
}
