//! Network and package operations stay off the UI and input threads. Only
//! installed builds can update; an unpackaged exe reports that explicitly.
use std::sync::mpsc::{self, Receiver, Sender};

pub const RELEASES: &str = "https://github.com/Dananz/mooz/releases";

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Unavailable,
    Idle,
    Checking,
    Current,
    Available(String),
    Downloading,
    Ready(String),
    Installing,
    Error(String),
}

impl Status {
    pub fn busy(&self) -> bool {
        matches!(self, Self::Checking | Self::Downloading | Self::Installing)
    }
}

enum Command {
    Check,
    Download,
    Install,
}

pub struct Updater {
    pub status: Status,
    commands: Sender<Command>,
    events: Receiver<Status>,
}

impl Updater {
    pub fn new() -> Self {
        let (commands, rx) = mpsc::channel();
        let (tx, events) = mpsc::channel();
        #[cfg(windows)]
        std::thread::spawn(move || worker(rx, tx));
        #[cfg(not(windows))]
        {
            let _ = (rx, tx);
        }
        Self {
            status: Status::Unavailable,
            commands,
            events,
        }
    }

    pub fn poll(&mut self) {
        while let Ok(status) = self.events.try_recv() {
            self.status = status;
        }
    }

    pub fn check(&mut self) {
        self.send(Command::Check, Status::Checking);
    }

    pub fn download(&mut self) {
        self.send(Command::Download, Status::Downloading);
    }

    pub fn install(&mut self) {
        self.send(Command::Install, Status::Installing);
    }

    fn send(&mut self, command: Command, status: Status) {
        if self.status.busy() || self.status == Status::Unavailable {
            return;
        }
        if self.commands.send(command).is_ok() {
            self.status = status;
        }
    }
}

#[cfg(windows)]
fn worker(commands: Receiver<Command>, events: Sender<Status>) {
    use velopack::{sources::GithubSource, UpdateCheck, UpdateManager};
    // Public stable releases only. No embedded GitHub token or custom feed URL.
    let source = GithubSource::new("https://github.com/Dananz/mooz", None, false);
    let Ok(manager) = UpdateManager::new(source, None, None) else {
        return;
    };
    let mut available = None;
    let mut downloaded = manager.get_update_pending_restart();
    let initial = downloaded
        .as_ref()
        .map_or(Status::Idle, |a| Status::Ready(a.Version.clone()));
    if events.send(initial).is_err() {
        return;
    }
    while let Ok(command) = commands.recv() {
        let result: Result<Status, String> = (|| match command {
            Command::Check => {
                // Keep a verified, downloaded package available for restart.
                if let Some(asset) = &downloaded {
                    return Ok(Status::Ready(asset.Version.clone()));
                }
                match manager.check_for_updates().map_err(|e| e.to_string())? {
                    UpdateCheck::UpdateAvailable(info) => {
                        let version = info.TargetFullRelease.Version.clone();
                        available = Some(info);
                        Ok(Status::Available(version))
                    }
                    _ => {
                        available = None;
                        Ok(Status::Current)
                    }
                }
            }
            Command::Download => {
                let info = available
                    .as_ref()
                    .ok_or("Check for updates again before downloading.")?;
                manager
                    .download_updates(info, None)
                    .map_err(|e| e.to_string())?;
                downloaded = Some(info.TargetFullRelease.clone());
                Ok(Status::Ready(info.TargetFullRelease.Version.clone()))
            }
            Command::Install => {
                let asset = downloaded.as_ref().ok_or("Download an update first.")?;
                // The settings process owns this operation. Stop the tray and
                // release its input hooks before Update.exe replaces binaries.
                crate::win::stop_tray_for_update().map_err(|e| e.to_string())?;
                manager
                    .wait_exit_then_apply_updates(asset, false, true, Vec::<String>::new())
                    .map_err(|e| e.to_string())?;
                std::process::exit(0);
            }
        })();
        if events.send(result.unwrap_or_else(Status::Error)).is_err() {
            break;
        }
    }
}
