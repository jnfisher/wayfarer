# wayfarer-parse

The game's terminal text to a stream of named events with fields (docs/plan/03-engine.md
section 1). A port of the prototype's `test/play/patterns.py` (jnfisher/mmud-exploration),
checked line for line against it (`tests/golden.rs`).

```rust
let texts = GameTexts::from_export("tests/fixtures/world")?;   // or the world pack, later
let reg = Arc::new(Registry::new(&texts));                     // ~100 ms, share it
let mut parser = Parser::new(reg);                              // one per session
for ev in parser.feed(chunk) { /* ev.name, ev.fields, ev.line, ev.seq, ev.line_no */ }
parser.sent("a rat");                                          // so its echo is `echo`
```

- **Patterns**: the static table (`tests/fixtures/parse-golden/patterns.json`, embedded at
  compile time) and about 6,400 patterns generated at load time from the world's WCCMSG
  records: monster arrivals, deaths, hits and misses, weapon verbs, spell texts, and every
  other game message (`wccmsg`). Room names make `room_title`.
- **Matching**: each pattern is indexed by its rarest literal word; a line is tried only
  against the patterns keyed by one of its words, plus about a hundred always tried, best
  priority first. Regexes compile on first use.
- **Incremental**: a line is classified when it is complete; a prompt (no newline) when a
  chunk ends with it. Wrapped displays (exits, `You notice`, `Also here`, `You are carrying`,
  keys) wait for the line that ends them. Any split of the text gives the same events, except
  a prompt cut before the end of its resting mark (reported twice; see `Parser`).
- **Prompt**: the standard form, `set statline custom` templates, the generic `[...hp=...]:`
  form and statline OFF (`PromptParser`).
- **Every line is an event**: lines nothing recognises are `unknown` (`unclassified`), or
  `room_desc` after a room title.
- `helpers`: splitting exits, the inventory list and `Also here`.

Python regex sources are kept as the prototype builds them (specificity and index keys are
computed on that text) and translated to the `regex` crate's syntax to compile (`pyre.rs`).

Tests: `cargo nextest run -p wayfarer-parse`. Speed, by hand:
`cargo test --release -p wayfarer-parse --test speed -- --ignored --nocapture`.
