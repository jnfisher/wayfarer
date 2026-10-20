//! The brain scenario (M1): a `wayfarer brain` process attached to the check's gateway.

use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::process::{Child, Command};

use crate::gateway::{keep_lines, terminate};

/// How long a brain may take to exit once told to.
const EXIT_TIMEOUT: Duration = Duration::from_secs(10);

/// Build the world pack from the fixtures' stock-world export into `dir`.
pub async fn world_pack(binary: &Path, dir: &Path) -> Result<PathBuf, String> {
    let export = Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/world");
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let out = dir.join("world.sqlite");
    let status = Command::new(binary)
        .arg("import-world")
        .arg("--export")
        .arg(&export)
        .arg("--out")
        .arg(&out)
        .stdout(Stdio::null())
        .status()
        .await
        .map_err(|e| format!("starting {}: {e}", binary.display()))?;
    if !status.success() {
        return Err(format!("wayfarer import-world exited with {status}"));
    }
    Ok(out)
}

/// A running `wayfarer brain`.
pub struct Brain {
    child: Child,
    log: Arc<Mutex<Vec<String>>>,
    /// How it was started, for [`Brain::restart`].
    binary: PathBuf,
    dir: PathBuf,
    addr: String,
    world: PathBuf,
    extra: Vec<String>,
}

impl Brain {
    /// Start `binary brain` against the gateway at `addr`, logging to `dir/<label>.log`.
    pub async fn start(
        binary: &Path,
        dir: &Path,
        label: &str,
        addr: &str,
        world: &Path,
    ) -> Result<Self, String> {
        Self::start_with(binary, dir, label, addr, world, &[]).await
    }

    /// The same, with `extra` arguments for `wayfarer brain` (`--flee-below 100`).
    pub async fn start_with(
        binary: &Path,
        dir: &Path,
        label: &str,
        addr: &str,
        world: &Path,
        extra: &[String],
    ) -> Result<Self, String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let mut child = Command::new(binary)
            .arg("brain")
            .arg("--gateway")
            .arg(format!("ws://{addr}/ws"))
            .arg("--world")
            .arg(world)
            // An empty data directory: no token file (the gateway is on loopback).
            .arg("--config")
            .arg(dir.join("wayfarer.toml"))
            .args(extra)
            .env("NO_COLOR", "1")
            .env("RUST_LOG", "info")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("starting {}: {e}", binary.display()))?;
        let log = Arc::new(Mutex::new(Vec::new()));
        let file = tokio::fs::File::create(dir.join(format!("{label}.log")))
            .await
            .map_err(|e| e.to_string())?;
        let file = Arc::new(tokio::sync::Mutex::new(file));
        let (addr_tx, _) = tokio::sync::mpsc::channel(1);
        keep_lines(
            child.stdout.take().expect("piped"),
            Arc::clone(&log),
            Arc::clone(&file),
            addr_tx.clone(),
        );
        keep_lines(
            child.stderr.take().expect("piped"),
            Arc::clone(&log),
            file,
            addr_tx,
        );
        Ok(Self {
            child,
            log,
            binary: binary.to_owned(),
            dir: dir.to_owned(),
            addr: addr.to_owned(),
            world: world.to_owned(),
            extra: extra.to_vec(),
        })
    }

    /// Stop it (SIGTERM) and start it again in the same directory, so on the same store,
    /// logging to `dir/<label>.log`: how it exited, and the new brain.
    pub async fn restart(self, label: &str) -> Result<(String, Self), String> {
        let (binary, dir, addr, world, extra) = (
            self.binary.clone(),
            self.dir.clone(),
            self.addr.clone(),
            self.world.clone(),
            self.extra.clone(),
        );
        let (status, _) = self.stop().await?;
        if !status.success() {
            return Err(format!("the first brain exited with {status}"));
        }
        let again = Self::start_with(&binary, &dir, label, &addr, &world, &extra).await?;
        Ok((format!("exited with {status}"), again))
    }

    /// Wait for it to exit by itself; its status and log.
    pub async fn exited(mut self) -> Result<(ExitStatus, String), String> {
        let status = tokio::time::timeout(EXIT_TIMEOUT, self.child.wait())
            .await
            .map_err(|_| format!("still running after {} s", EXIT_TIMEOUT.as_secs()))?
            .map_err(|e| e.to_string())?;
        // Let the log reader catch the last lines.
        tokio::time::sleep(Duration::from_millis(100)).await;
        let log = self.log.lock().unwrap().join("\n");
        Ok((status, log))
    }

    /// Its log so far.
    pub fn log(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }

    /// Stop it as Ctrl-C would (SIGTERM) and wait for it to exit.
    pub async fn stop(self) -> Result<(ExitStatus, String), String> {
        terminate(self.child.id());
        self.exited().await
    }
}

impl Drop for Brain {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            terminate(self.child.id());
        }
    }
}

/// The HP of the last standard prompt in `transcript` (`[HP=35]:`, `[HP=-3/MA=22]:`).
pub fn last_prompt_hp(transcript: &str) -> Option<i64> {
    let rx = regex::Regex::new(r"\[HP=(-?\d+)").expect("compiles");
    rx.captures_iter(transcript)
        .last()
        .and_then(|c| c[1].parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_prompt_wins() {
        let t = "[HP=35]:st\nHits: 35/35\n[HP=-3/MA=22]:\nYou feel better.";
        assert_eq!(last_prompt_hp(t), Some(-3));
        assert_eq!(last_prompt_hp("Username:"), None);
    }
}
