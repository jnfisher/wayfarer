//! Where a session's settings come from: the files in the settings directory when there is one
//! (resolved again when the character's class becomes known, for the class presets), else the
//! brain's fixed settings (docs/brain.md "Settings").

use std::sync::Arc;

use wayfarer_settings::{Settings, SettingsDir};

use crate::policy::FLEE_BELOW;

#[derive(Debug, Clone)]
pub(crate) struct SettingsSource {
    dir: Option<SettingsDir>,
    /// The game server as the gateway names it (`host:port`), for the server layer's file.
    server: String,
    /// What the session uses when there is no directory, or its files are not valid.
    fallback: Arc<Settings>,
    /// The live check's `--flee-below`, which beats the settings.
    flee_override: Option<i32>,
}

impl SettingsSource {
    pub fn new(
        dir: Option<SettingsDir>,
        server: String,
        fallback: Arc<Settings>,
        flee_override: Option<i32>,
    ) -> Self {
        Self {
            dir,
            server,
            fallback,
            flee_override,
        }
    }

    /// Whether the settings come from files, which the class presets can move; fixed settings
    /// (a test's) are used as they are.
    pub fn reads_files(&self) -> bool {
        self.dir.is_some()
    }

    /// The settings for `character` of `class` (when known), and the warning if their files
    /// could not be used, in which case the fallback applies.
    pub fn resolve(&self, character: &str, class: Option<&str>) -> (Arc<Settings>, Option<String>) {
        let Some(dir) = &self.dir else {
            return (self.fallback.clone(), None);
        };
        match dir.resolve_for(&self.server, character, class) {
            Ok(s) => (Arc::new(s), None),
            Err(e) => (
                self.fallback.clone(),
                Some(format!("Settings not used, the defaults apply: {e}")),
            ),
        }
    }

    /// The flee line: the knob's, else the setting `health.flee_below_hp`.
    pub fn flee_below(&self, settings: &Settings) -> i32 {
        self.flee_override.unwrap_or_else(|| {
            i32::try_from(settings.int("health.flee_below_hp")).unwrap_or(FLEE_BELOW)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(dir: Option<&std::path::Path>, flee: Option<i32>) -> SettingsSource {
        SettingsSource::new(
            dir.map(SettingsDir::new),
            "127.0.0.1:2323".into(),
            Arc::new(Settings::default()),
            flee,
        )
    }

    #[test]
    fn fixed_settings_ignore_the_class() {
        let s = source(None, None);
        assert!(!s.reads_files());
        let (settings, warning) = s.resolve("thorgrim", Some("Mage"));
        assert_eq!(warning, None);
        assert_eq!(settings.int("health.flee_below_hp"), 25);
        assert_eq!(s.flee_below(&settings), 25);
    }

    #[test]
    fn files_are_resolved_for_the_class_and_the_knob_beats_them() {
        let dir = tempfile::tempdir().unwrap();
        let s = source(Some(dir.path()), None);
        assert!(s.reads_files());
        let (fighter, _) = s.resolve("thorgrim", None);
        let (mage, _) = s.resolve("thorgrim", Some("Mage"));
        assert_eq!(s.flee_below(&fighter), 25);
        assert_eq!(s.flee_below(&mage), 45);
        let knob = source(Some(dir.path()), Some(80));
        assert_eq!(knob.flee_below(&mage), 80);
    }

    #[test]
    fn a_file_that_is_not_valid_falls_back_with_a_warning() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("settings.toml"),
            "[health]\nflee_below_hp = 95\n",
        )
        .unwrap();
        let (settings, warning) = source(Some(dir.path()), None).resolve("x", Some("Mage"));
        assert_eq!(settings.int("health.flee_below_hp"), 25);
        let warning = warning.unwrap();
        assert!(warning.starts_with("Settings not used"), "{warning}");
        assert!(warning.contains("health.flee_below_hp is 95"), "{warning}");
    }
}
