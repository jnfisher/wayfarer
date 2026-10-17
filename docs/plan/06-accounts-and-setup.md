# 06. Accounts, characters and first run

## The pain today

Getting one character playing takes: a BBS account (the `NEW` dialogue: name, password twice,
e-mail, gender), a logon script for the client (MegaMud wanted the BBS's sign-on prompts and
replies typed into a dialog), a character (race and class from numbered lists, the lawful
question, then the full-screen stat editor), and settings. A party multiplies that by six, and
each character needs its own BBS account, since a server allows one connection per user
(`kb/connection.md`).

## First run

A short wizard, every step skippable:

1. **Server.** Presets: "silvermere on this computer" (`localhost:2323`), MBBSEmu, or a host
   and port. Later, a curated list of public MajorMUD BBSes with their logon presets. The
   client connects and detects the server kind from its greeting.
2. **Characters.** "I already have characters": enter account name and password for each;
   the client logs on, reads the character (`st`), and offers class defaults for its settings.
   "Make new ones": steps 3 and 4.
3. **BBS accounts, made for you.** The client runs the `NEW` dialogue itself: the name the
   player chose, a password generated within the server's rules (MBBSEmu: 5 to 9 characters,
   not `password`) or typed, a placeholder e-mail, gender. Refusals are shown in plain words
   and the step repeats.
4. **Character creation, handed to the player.** The wizard drives the game up to the stat
   editor and then the player takes over in the character's tile:
   - race and class are picked in the wizard, with a summary of each class guide
     (`kb/classes/`) and its recommended races; the client answers the game's numbered lists
     and the lawful question;
   - at MajorMUD's full-screen stat editor the client stops: the player types the names and
     distributes CP in the game's own editor (raw keys go straight through; the bot is
     stopped). Beside the tile, a panel shows the class guide's advice and the stat ladder
     `statplan.py` would suggest, as reading only;
   - when the editor closes with SAVE and the character is in the realm, the wizard carries on.
5. **Settings.** Class defaults applied; the essentials page shown once (`04-settings.md`).
6. **Go.** The character enters the realm in a tile with its bot stopped. The card offers
   "Where next?" (`03-engine.md`, section 4); the player picks an area and presses start.

**A party in one go.** "New party" asks for up to six names and classes (with suggested
compositions from `kb/grouping.md`) and runs steps 3 to 5 for each.

## Logon scripts

A logon script is a list of `prompt → reply` steps from connect to the `[HP=..]:` prompt. The
client ships presets for silvermere, MBBSEmu and Worldgroup's standard prompts, and detects
which applies. For an unusual BBS, **learn by doing**: the player logs on by hand once, and the
client records the prompts it saw and the replies typed (passwords stored as a reference to
the credential, never as text), producing a script the player can review.

The gateway runs logon scripts, so reconnects work without the brain. On a lost connection it
reconnects with backoff. On quit it logs characters off cleanly (`x`, then `X`, `X`, `Y`),
because dropping in the realm is punished on the next entry (`kb/connection.md`).

## Credentials

Stored locally for now: the BBS account names and passwords go in one file in the user's data
directory (`credentials.toml`, readable only by the user on Linux and macOS), separate from the
settings files so profiles can be exported and shared without them. No keychain integration
yet: it adds a platform dependency per OS for little gain on a personal machine. Revisit
before a public release.

## Migration from MegaMud

If a player has a MegaMud setup, import what can be read from its files (character settings,
BBS logon prompts, recorded paths). What MegaMud's files hold is in `kb/megamud.md`; this is a
later card, not a first-wave one.
