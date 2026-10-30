---
id: C-062
title: "Accounts panel: edit servers, characters and logins in the page; Save applies them live and makes missing BBS accounts"
tier: B
size: L
wave: 11
depends: []
crate: wayfarer-gateway, wayfarer-proto, ui
touches:
  - crates/wayfarer-gateway/src/{config.rs,gateway.rs,logon.rs,server.rs,session.rs}, crates/wayfarer-gateway/src/presets/*.toml
  - crates/wayfarer-proto/src/lib.rs
  - crates/wayfarer-brain/ (only if it cannot take sessions that come and go; see Notes)
  - ui/src/AccountsPanel.svelte (new), ui/src/{App.svelte,Help.svelte}, ui/src/lib/{gateway.ts,keys.ts,accounts.ts (new)}
  - docs/web-ui.md, docs/gateway.md
---

## Goal

John sets up characters from the page instead of by editing `wayfarer.toml` and
`credentials.toml`. The work is mostly done when he makes a new character: he adds a row with
a name, server, BBS user and password, then presses **Save**. The files are written, a tile
appears and logs on. If the BBS doesn't know the user, the gateway creates the account
(`NEW`) and stops at the MajorMUD menu, where John makes the character in the tile himself.
Characters he didn't touch keep playing.

## Inputs

- John's request (2026-10-30). This card is the part of `docs/plan/06-accounts-and-setup.md`
  "First run" steps 2 and 3 that he asked for. It is not the wizard: there is no server
  detection, no generated passwords and no character creation.
- `crates/wayfarer-gateway/src/config.rs` (`Config`, `Server`, `Character`, `Credentials`,
  `Paths`; `credentials.toml` is created mode 0600), `examples/wayfarer.toml`,
  `examples/credentials.toml`, `docs/gateway.md`.
- `crates/wayfarer-gateway/src/gateway.rs`: today the sessions are built once in
  `Gateway::with_connector` from the config, and the set never changes after that.
- `crates/wayfarer-gateway/src/logon.rs` and `src/presets/{mbbsemu,silvermere}.toml`: the
  logon script, its `failure` table (`Invalid Credentials` → "bad credentials", no retry).
- The sign-up dialogue:
  - `docs/kb/connection.md` "Login flow (MBBSEmu)": `NEW`, then the username, the password
    twice, the e-mail and the gender `M`/`F`; refusals for a blank, over-29-character or
    taken name, a password that is not 5 to 9 characters or is `password`, a bad e-mail, and
    anything but `M`/`F` ([I], from MBBSEmu's `MenuRoutines.cs`).
  - silvermere's prompts, from `silvermere/crates/silvermere-auth/tests/login_dialogue.rs`
    (read only): `Please enter a unique Username (Max. 29 Characters):`,
    `Please enter a strong Password:`, `Please re-enter your password to confirm:`,
    `Please enter a valid e-Mail Address:`,
    `Please enter your gender 'M' or 'F' (can be changed later):`. A refusal prints a line
    (`That Username is unavailable, please choose another.` and others) and asks **the same
    question again**. silvermere locks an account after repeated bad passwords
    (`Too many failed logins`).
- `ui/src/App.svelte` (`DRAWER_BUTTONS`: Map, Parties, Blessings, Stats, Settings, Dev) and
  `ui/src/PartiesPanel.svelte`, the closest model for a panel that edits definitions and
  saves them through a message.

## Behaviour

**The panel.** It is a new drawer panel, **Accounts**, opened from a button in the drawer bar
beside the others or with a free `Alt` key (`keys.ts` and the help overlay). It shows:

- **Servers**: one row each with id, host, port and logon preset (a dropdown of the presets).
  There is **+ Server**, and delete. A server that a character still uses can't be deleted;
  the panel says which characters use it.
- **Characters**: one row each with:
  - the display name (`name`);
  - the id, filled from the name (lower-cased, anything outside `[a-z0-9_-]` dropped), which
    can be edited only until the first save;
  - the server (a dropdown);
  - the BBS user;
  - the password, shown as dots with a show/hide toggle;
  - autostart;
  - the session's live state badge, as on the tile;
  - remove.

  There is also **+ Character**. A new row starts with gender **F** and e-mail
  **h@h.com**, both editable, and both used only when the account has to be made. They are
  not written to either file.
- **Save** and **Revert**. Save is enabled when something has changed. The panel checks
  what `Config::check` checks before sending: ids, duplicates, a character's server existing,
  a non-empty host and a non-zero port, and up to six characters (the tile layout's limit).
  Errors show on their row.

**Save.** The gateway writes both files and then applies the difference:

- **Writing.** `wayfarer.toml` and `credentials.toml` are each written to a temporary file
  and renamed over the old one. `credentials.toml` keeps mode 0600. The old `wayfarer.toml`
  is copied to `wayfarer.toml.bak` first. Comments in the files are not kept: Save writes
  the files fresh, and the files say so in a header comment. A `[server.logon]` inline
  script, which the panel doesn't edit, is kept as it was. A write that fails changes
  nothing and the panel shows why.
- **New character.** A session is added, its tile appears in every open page, and it
  connects whether or not autostart is ticked (see "Making the account" below).
- **Changed character** (server, user or password): it disconnects at once and connects
  again with the new details (see "Hanging up" below).
- **Only the name or autostart changed**: no reconnect. The tile's name updates, and the new
  autostart applies the next time the gateway starts.
- **Removed character**: it disconnects at once and its tile goes away. Its logs, its
  `characters/<id>.toml` settings and its store rows stay on disk.
- **Changed server** (host, port or preset): every character on it counts as changed.
- **Everything else** keeps playing, untouched: not reconnected, not paused.

**Hanging up.** A reconnect or a removal drops the connection; it doesn't run the clean
logoff. That is the gateway's default (`bot.log_off = hangup`, CLAUDE.md "Game facts that
become guards"), and it is what John asked for. The panel's Save confirm says which
characters will be dropped, for example *"Save: Faye and Manuel will hang up and log on
again."*

**Making the account.** A new character, or a changed one whose user changed, logs on with
the normal script. When the BBS answers `Invalid Credentials` for that user, the gateway
reconnects (silvermere closes after failures, and MBBSEmu returns to `Username:`) and runs
the sign-up:

1. `NEW` at `Username:`.
2. The user at the username prompt.
3. The password at the password prompt, and again at the confirmation prompt.
4. The e-mail, then the gender.
5. After the gender, the script carries on from the BBS menu as usual (`M`), but **stops at
   `[MAJORMUD]:`**. It doesn't send `E`. The session's state is `menu` and nothing else is
   sent. The brain doesn't start anything on a session that isn't in the realm.

If any sign-up question comes back a second time, the server refused the answer. The gateway
stops, disconnects, and reports the refusal line it saw, for example *"That Username is
unavailable, please choose another."*, as the session's reason. The panel shows it on the
row. Nothing is retried on its own.

A user that *does* log on is left alone. It is never signed up again, and a wrong password
on an existing account stays "bad credentials" and is never retried as `NEW`. Only the
first `Invalid Credentials` after Save, for a character that is new or whose user changed,
leads to sign-up. A character whose password changed but not its user gets "bad
credentials" and no sign-up.

**Stopping at the menu after sign-up.** This applies to the first connection after a
sign-up only. Later connections (a reconnect, a restart, John pressing connect) run the
full script, `E` included. John makes the character from the menu by typing `E` in the tile
and going through the game's creation himself.

**Messages** (in `wayfarer-proto`; names are the builder's call):

- UI → gateway: one "get accounts" request.
- Gateway → UI: an accounts message with the servers, the characters and their logins,
  passwords included. Send it only to UI clients on a loopback bind, or on a bind that needs
  the token. Never send it to the brain.
- UI → gateway: "save accounts" with the whole edited set, and the gender and e-mail for new
  rows.
- Gateway → UI: a reply with ok, or the error. The new session list goes to every client.

## Design notes

- **Sessions that come and go.** `Gateway` gets `add_session`, `remove_session` and
  `replace_session` (or one `apply(Config, Credentials, signup)`), built on what `new` does
  per character today. The diff is computed in `config.rs` as a pure function
  (`fn diff(old, new) -> Changes`), which is where most of the tests go.
- **Sign-up as script data, not code.** Add a `[[signup]]` table to each preset with the
  same step shape (`expect`/`send`, `{user}`, `{password}` and the new `{email}` and
  `{gender}`) plus a `stop_at` pattern (`[MAJORMUD]:`). A preset without one can't sign up,
  and the row says *"this server's logon script can't make accounts"*. The re-ask rule
  ("a question seen twice is a refusal") lives in the logon runner, which reports the line
  before the repeated prompt.
- **Writing TOML.** `Config` and `Credentials` only implement `Deserialize` today. Add
  `Serialize` and write with `toml::to_string`. A `toml_edit` round trip that keeps comments
  isn't needed.
- **Passwords**: never logged, never in a `Debug`, never in the session's output log as text
  (the password echoes as `*`, but what the gateway sends goes to `sent.bin`; check that
  sign-up sends are treated the way logon password sends already are).
- Out of scope: the clean logoff on Save, editing bot settings (the Settings panel does
  that), creating the MajorMUD character, server detection, generated passwords, MegaMud
  import, and more than six characters.

## Done when

- On the live silvermere, John adds a character with a user that doesn't exist, presses Save,
  and sees: the files written (`credentials.toml` mode 0600), a new tile logging on, the
  account made, and the tile stopped at `[MAJORMUD]:`. He then types `E` and makes the
  character by hand.
- During the same Save, two other characters already in the realm keep playing, with no
  reconnect in their logs.
- Changing one character's password hangs that character up and reconnects it. Removing one
  drops its tile and leaves its files.
- A taken username shows silvermere's refusal on the row, and the session is closed.
- Restarting the gateway after any of these reads the same files without error.
- `docs/web-ui.md` has an "Accounts" section, and `docs/gateway.md` says the files can be
  written by the page and that Save drops comments.
- fmt, clippy, nextest, `npm run check` and `npm test` pass.

## Tests

- `config.rs`: `diff` for each case (new, removed, user, password, server host, name only,
  autostart only, untouched). Also a write-then-load round trip of both files, the 0600 mode,
  the `.bak` copy, an inline `[server.logon]` kept, and a failed write leaving the old files.
- `logon.rs`, fed text the way the existing tests are:
  - the sign-up against silvermere's prompts and MBBSEmu's, stopping at `[MAJORMUD]:`
    without `E`;
  - a refused username, password and e-mail (the question seen twice gives the refusal line
    as the reason);
  - `Invalid Credentials` on an unchanged character gives "bad credentials" and no sign-up.
- `gateway.rs`, with the in-process connector: apply a config that adds, replaces and
  removes sessions while a fourth keeps its connection, and check the state changes and the
  session list sent to subscribers.
- The UI's `accounts.ts`: the id made from a name, the client-side checks, which rows Save
  would reconnect, and the confirm text. A component test that Save sends the edited set.
- `scripts/live-check.sh` gets an opt-in step that signs up a random throwaway user on
  silvermere (it only runs live, never in CI).

## Notes for the agent

- **Brain and parties.** Check that the brain copes with a session list that changes while
  it runs: it should start tracking a new id, and drop a removed one, including from a party
  definition, where the member is shown as missing and not crashed on. If it doesn't, the
  smallest fix belongs in this card. Say what you did in the PR.
- **A partly made character.** After sign-up, John may leave the game's creation half done.
  The next full logon sends `E` and lands back in creation, and the script waits for `[HP=`
  and times out. Make that timeout leave the session open, in state `menu` with the reason
  *"no character yet: finish making it in the tile"*, instead of reconnecting in a loop.
  Check what silvermere shows there and write the fact into `docs/kb/connection.md` with its
  tag.
- **The KB.** If the live run confirms MBBSEmu's or silvermere's sign-up prompts word for
  word, update `docs/kb/connection.md` (it has only MBBSEmu's from source, [I]) and
  `docs/kb/changelog.md`.
- **A dev brain watching the checkout.** John's dev brain restarts when files under
  `crates/` change in `~/repos/wayfarer`. Work in your worktree only.
- **This repository is public.** No real hostnames, users or passwords go in tests,
  fixtures or docs. The defaults `F` and `h@h.com` are fine.
