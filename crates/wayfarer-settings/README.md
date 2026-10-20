# wayfarer-settings

Settings declared once (`docs/plan/04-settings.md`). A leaf crate: it depends on `toml` only.

- **`CATALOGUE`**: every `Setting` with its key (`group.name`), label, help sentence, kind and
  range, default, unit, tier, group and MegaMud equivalent. A test fails if one lacks help, a
  unit, or a default inside its range, or is declared twice.
- **`Overrides`**: the values one layer sets, and only those. `Overrides::parse` reads a TOML
  file (one table per group) and refuses an unknown key, a wrong type or a number out of range;
  `to_toml` writes it back.
- **`Settings::resolve(&[(Layer, &Overrides)])`**: the layers applied least specific first
  (`Default < Mine < Server < Character`), with `source(key)` saying which layer a value came
  from. The HP lines must run flee below < rest below <= rest until.
- **`int(key)` / `bool(key)`**: the value in force. The brain reads them from its `Atlas`
  (`Brain::with_settings`); `wayfarer brain` and `wayfarer serve` load `settings.toml` from the
  data directory.

```toml
# settings.toml, in the data directory; only what differs from the defaults
[health]
flee_below_hp = 35     # default 30
rest_until_hp = 90     # default 95

[movement]
search_limit = 500     # default 300
```

First slice: five settings, one file for all characters. The forms, class and style presets,
per-server and per-character files, the narration of changes and the MegaMud import are the
rest of `docs/plan/04-settings.md`.
