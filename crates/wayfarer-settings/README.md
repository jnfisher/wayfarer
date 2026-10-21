# wayfarer-settings

Settings declared once (`docs/plan/04-settings.md`). A leaf crate: it depends on `toml` only.

- **`CATALOGUE`**: every `Setting` with its key (`group.name`), label, help sentence, kind (a
  whole number in a range, a switch, or one of some words) and default, unit, tier, group and
  MegaMud equivalent. A test fails if one lacks help, a unit, or a default inside its range, or
  is declared twice.
- **`Overrides`**: the values one layer sets, and only those. `Overrides::parse` reads a TOML
  file (one table per group) and refuses an unknown key, a wrong type, a word that is not an
  option or a number out of range; `to_toml` writes it back.
- **`Settings::resolve(&[(Layer, &Overrides)])`**: the layers applied least specific first
  (`Default < Style < Mine < Server < Character`), with `source(key)` saying which layer a value
  came from. The style (`bot.style`: careful, balanced, bold) is the most specific one named;
  its `preset` of the three HP lines sits under every layer given. The HP lines must run flee
  below < rest below <= rest until.
- **`SettingsDir`**: the files that are the layers, in the data directory: `settings.toml`,
  `servers/<host>_<port>.toml` and `characters/<character id>.toml`. A missing file is an empty
  layer; an invalid one is a `FileError` naming it. Ids that are not safe as file names name no
  file.
- **`int(key)` / `bool(key)` / `choice(key)`** read the value in force; `rows()` lists every
  setting with its value and source (what `wayfarer settings` prints).

```toml
# characters/thorgrim.toml: only what differs
[bot]
style = "bold"      # flee 20, rest below 50, rest until 80 (the preset)

[health]
rest_until_hp = 92  # beats the preset

[movement]
search_limit = 500  # default 300
```

Second slice: nine settings, the style presets, per-server and per-character files. The forms,
the other class presets, narrating a change, applying one to a running bot and the MegaMud import are the
rest of `docs/plan/04-settings.md`.
