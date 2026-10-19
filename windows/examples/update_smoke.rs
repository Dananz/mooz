//! CI-only driver. Not included in the installer. Exercises Velopack's real
//! check/download/apply APIs against a local feed without publishing a release.
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::path::PathBuf;
    use velopack::{locator::VelopackLocatorConfig, sources::FileSource, UpdateCheck, UpdateManager};
    let mut args = std::env::args().skip(1);
    let root = PathBuf::from(args.next().ok_or("installation root required")?);
    let feed = args.next().ok_or("feed directory required")?;
    let expected = args.next().ok_or("expected target version required")?;
    let locator = VelopackLocatorConfig {
        RootAppDir: root.clone(),
        UpdateExePath: root.join("Update.exe"),
        PackagesDir: root.join("packages"),
        ManifestPath: root.join("current/sq.version"),
        CurrentBinaryDir: root.join("current"),
        IsPortable: false,
    };
    let manager = UpdateManager::new(FileSource::new(&feed), None, Some(locator))?;
    let UpdateCheck::UpdateAvailable(info) = manager.check_for_updates()? else {
        return Err("expected an update".into());
    };
    assert_eq!(info.TargetFullRelease.Version, expected);
    manager.download_updates(&info, None)?;
    println!("Downloaded and verified {}", info.TargetFullRelease.Version);
    manager.wait_exit_then_apply_updates(&*info, true, false, Vec::<String>::new())?;
    Ok(())
}

#[cfg(not(windows))]
fn main() {}
