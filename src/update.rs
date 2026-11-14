//! Checks GitHub Releases without holding up the message window.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::os::windows::process::CommandExt;

use serde::Deserialize;

const RELEASES_URL: &str = "https://github.com/william-pyle/MessageCraft/releases/latest";

#[derive(Clone, Deserialize)]
pub struct Release {
    pub version: String,
    pub url: String,
    #[serde(default)]
    pub digest: String,
}

pub enum Event {
    Available(Release),
    Current,
    Downloaded(PathBuf),
    Failed(String),
}

pub fn script_path() -> PathBuf {
    let beside_exe = std::env::current_exe().unwrap_or_default().with_file_name("update.ps1");
    if beside_exe.is_file() { beside_exe } else { PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("update.ps1") }
}

fn powershell(args: &[&str]) -> Result<String, String> {
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(script_path())
        .args(args)
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .output()
        .map_err(|e| format!("Could not start the update check: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn check() -> Receiver<Event> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let result = powershell(&["-Action", "check"])
            .and_then(|json| serde_json::from_str::<Release>(&json).map_err(|e| e.to_string()))
            .map(|release| if is_newer(&release.version, env!("CARGO_PKG_VERSION")) { Event::Available(release) } else { Event::Current })
            .unwrap_or_else(Event::Failed);
        let _ = tx.send(result);
    });
    rx
}

pub fn download(release: Release) -> Receiver<Event> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let result = powershell(&["-Action", "download", "-Version", &release.version, "-Url", &release.url, "-Digest", &release.digest]);
        let _ = tx.send(match result {
            Ok(path) if PathBuf::from(&path).is_file() => Event::Downloaded(PathBuf::from(path)),
            Ok(_) => Event::Failed("The update download did not produce an installer.".into()),
            Err(e) => Event::Failed(format!("Update download failed: {e}")),
        });
    });
    rx
}

pub fn install(path: &std::path::Path) -> Result<(), String> {
    Command::new("msiexec.exe")
        .arg("/i")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not open the update installer: {e}"))
}

pub fn open_releases() {
    let _ = Command::new("explorer.exe").arg(RELEASES_URL).spawn();
}

fn is_newer(candidate: &str, current: &str) -> bool {
    fn parts(s: &str) -> Option<[u64; 3]> {
        let s = s.trim_start_matches('v').split(['-', '+']).next()?;
        let mut nums = s.split('.').map(str::parse::<u64>);
        let version = [nums.next()?.ok()?, nums.next()?.ok()?, nums.next()?.ok()?];
        nums.next().is_none().then_some(version)
    }
    matches!((parts(candidate), parts(current)), (Some(a), Some(b)) if a > b)
}

#[cfg(test)]
mod tests {
    use super::is_newer;

    #[test]
    fn compares_release_versions() {
        assert!(is_newer("v1.3.1", "1.3.0"));
        assert!(is_newer("v1.10.0", "1.9.9"));
        assert!(!is_newer("v1.3.0", "1.3.0"));
        assert!(!is_newer("v1.2.9", "1.3.0"));
        assert!(!is_newer("latest", "1.3.0"));
    }
}
