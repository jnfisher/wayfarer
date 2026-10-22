//! Where a session's settings come from: the files in the settings directory when there is one
//! (resolved again when the character's class becomes known, for the class presets), else the
//! brain's fixed settings (docs/brain.md "Settings").

use std::sync::Arc;

use wayfarer_proto::{SettingChange, SettingKind, SettingRow, SettingValue, SettingsFile};
use wayfarer_settings::{Kind, Layer, Settings, SettingsDir, Value};

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
        // A new character gets a starter file to edit (all comments, so it changes nothing);
        // one that cannot be written is not worth a warning.
        let _ = dir.write_starter("", "", false);
        if !character.is_empty() {
            let _ = dir.write_starter("", character, false);
        }
        match dir.resolve_for(&self.server, character, class) {
            Ok(s) => (Arc::new(s), None),
            Err(e) => (
                self.fallback.clone(),
                Some(format!("Settings not used, the defaults apply: {e}")),
            ),
        }
    }

    /// What the settings files look like now (their modification times and sizes, `None` for a
    /// file that is not there), to see that one changed. Empty without a directory.
    pub fn stamp(&self, character: &str) -> Vec<Option<(std::time::SystemTime, u64)>> {
        let Some(dir) = &self.dir else {
            return Vec::new();
        };
        dir.files(&self.server, character)
            .into_iter()
            .map(|(_, path)| {
                let meta = std::fs::metadata(path).ok()?;
                Some((meta.modified().ok()?, meta.len()))
            })
            .collect()
    }

    /// The settings from the files as they are now, or why they cannot be used: a reload
    /// keeps the settings in force then, unlike a first resolve, which falls back.
    pub fn reload(&self, character: &str, class: Option<&str>) -> Result<Arc<Settings>, String> {
        let dir = self.dir.as_ref().ok_or("no settings directory")?;
        dir.resolve_for(&self.server, character, class)
            .map(Arc::new)
            .map_err(|e| e.to_string())
    }

    /// Every setting as the forms show it: the value in force and where it comes from, and what
    /// the file of `layer` sets itself.
    pub fn rows(&self, settings: &Settings, character: &str, layer: Layer) -> Vec<SettingRow> {
        let own = self
            .dir
            .as_ref()
            .map(|d| d.read_layer(&self.server, character, layer));
        settings
            .rows()
            .map(|(s, value, source)| SettingRow {
                key: s.key.to_owned(),
                label: s.label.to_owned(),
                help: s.help.to_owned(),
                group: s.group.table().to_owned(),
                tier: s.tier.id().to_owned(),
                kind: match s.kind {
                    Kind::Int { min, max } => SettingKind::Int { min, max },
                    Kind::Bool => SettingKind::Bool,
                    Kind::Choice(options) => SettingKind::Choice {
                        options: options.iter().map(|o| (*o).to_owned()).collect(),
                    },
                },
                unit: s.unit.to_owned(),
                default: wire(s.default),
                value: wire(value),
                source: source.id().to_owned(),
                own: own.as_ref().and_then(|o| o.get(s.key)).map(wire),
                megamud: s.megamud.map(str::to_owned),
            })
            .collect()
    }

    /// The character's settings files.
    pub fn files(&self, character: &str) -> Vec<SettingsFile> {
        let Some(dir) = &self.dir else {
            return Vec::new();
        };
        dir.files(&self.server, character)
            .into_iter()
            .map(|(layer, path)| SettingsFile {
                layer: layer.id().to_owned(),
                exists: path.is_file(),
                path: path.display().to_string(),
            })
            .collect()
    }

    /// Change the file of `layer`: see [`SettingsDir::save`]. The wire values are matched to
    /// the declarations first (a word must be one of the options, a number a number).
    pub fn save(
        &self,
        character: &str,
        class: Option<&str>,
        layer: Layer,
        changes: &[SettingChange],
    ) -> Result<(), String> {
        let dir = self
            .dir
            .as_ref()
            .ok_or("the settings of this brain are fixed")?;
        let mut parsed = Vec::new();
        for c in changes {
            let value = match &c.value {
                Some(v) => Some(value_of(&c.key, v)?),
                None => None,
            };
            parsed.push((c.key.clone(), value));
        }
        dir.save(&self.server, character, class, layer, &parsed)
            .map(|_| ())
    }

    /// The flee line: the knob's, else the setting `health.flee_below_hp`.
    pub fn flee_below(&self, settings: &Settings) -> i32 {
        self.flee_override.unwrap_or_else(|| {
            i32::try_from(settings.int("health.flee_below_hp")).unwrap_or(FLEE_BELOW)
        })
    }
}

fn wire(v: Value) -> SettingValue {
    match v {
        Value::Int(n) => SettingValue::Int(n),
        Value::Bool(b) => SettingValue::Bool(b),
        Value::Choice(c) => SettingValue::Text(c.to_owned()),
    }
}

/// A wire value as the setting `key` takes it, or why it does not.
fn value_of(key: &str, v: &SettingValue) -> Result<Value, String> {
    let s = wayfarer_settings::setting(key).ok_or_else(|| format!("no setting {key:?}"))?;
    match (s.kind, v) {
        (Kind::Int { .. }, SettingValue::Int(n)) => Ok(Value::Int(*n)),
        (Kind::Bool, SettingValue::Bool(b)) => Ok(Value::Bool(*b)),
        (Kind::Choice(options), SettingValue::Text(t)) => options
            .iter()
            .find(|o| **o == t)
            .map(|o| Value::Choice(o))
            .ok_or_else(|| format!("{key} is {t:?}, not one of {}", options.join(", "))),
        (Kind::Int { .. }, _) => Err(format!("{key} wants a whole number")),
        (Kind::Bool, _) => Err(format!("{key} wants true or false")),
        (Kind::Choice(options), _) => Err(format!("{key} wants one of {}", options.join(", "))),
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
