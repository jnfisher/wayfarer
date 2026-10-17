# Capture: flood-and-refusals

Refusals: `search` (a one-tick command delay, LJNCMDA.C:3227) followed by fourteen `exp` typed without waiting, so the lines queue (LJNPEND.C:754-795): the 9th waiting line gets `Why don't you slow down for a few seconds?` and the 13th and later `You are typing too quickly - command ignored`; then `w` into a wall in the arena (`There is no exit in that direction!`), `n` at the closed door, `train` outside a training room, `rest` while a monster attacks (accepted: `You are now resting.`; the next blow ends it), the fight that follows, and logoff.

| | |
|---|---|
| Server | original `WCCMMUD.DLL` (sha256 `e308533696e1e64cf52715921d94d4245f4708f56ea048e4f7c6ec9b564b725f`) under MBBSEmu v1.0-alpha-090826, telnet 2323 |
| Captured | 2026-10-14 14:41 |
| Terminal | no NAWS answer (MBBSEmu default 80x24, ANSI on) |
| Character | user Aldric, name Aldric Stonebrook, race Human, class Warrior, level 1, start Newhaven, Narrow Road |
| raw.bin | 8841 bytes, 60 `ESC[79D`, 0 `\x01` |
| Steps | 35 (31 sends) |

## Notes

- The card asked for eight commands without waiting; eight instant commands never queue (each runs at once, no delay), so the burst starts with `search` and has fifteen lines.
- Commands typed while others wait are echoed and run one per tick; queued `exp` output interleaves with the monster's attacks.
- An earlier run of this script started in the wrong room (the healer's) and sent `a healer`: `You are overcome with a feeling of guilt and break off your attack.` That run was discarded.

## Pitfalls exercised

- `kb/pitfalls.md` 20: resting in a spawn room while attacked is accepted and broken by the next blow
- `kb/pitfalls.md` 24: the arena's closed north door
- `kb/pitfalls.md` 34: `There is no exit in that direction!`
- `kb/pitfalls.md` 36: `train` outside a training room
- survey section 5 item 11: no-exit refusal
- survey section 5 item 17: type-ahead echo: queued lines are echoed when they run
- survey section 5 item 18: flood control
- survey section 5 item 28: exit refusals

## Mask hints

Fields the comparer masks before a byte comparison (also in `script.json` `mask`):

- `\(Dec 30 2005 14:20:05\)`: build stamp of the DLL (mud_title)
- ` \[\d+ game minutes left\]| \[Last chance\]`: menu time-left suffix (LJNPRMPT.C:110)
- `[A-Z ]\x08`: every direction name carries a random capital letter and a backspace, rewritten each slow cycle (dirnames[], LJNTABLE.C:65-103): `nJ\borth`
- `\[HP=-?\d+(?:/(?:MA|KAI)=-?\d+)?`: HP/mana in the prompt depend on earlier fights and regen
- `for \d+ damage|You gain \d+ experience|\d+ (?:copper farthings?|silver nobles?|gold crowns?|platinum pieces?|runic coins?)`: RNG: damage, experience share, coin drops
- `giant rat|lashworm|filthbug|carrion beast|acid slime|kobold thief`: which monster spawned

## Unclassified lines

Lines of `plain.txt` the pattern registry reports as `unknown`. `tests/test_captures.py` allows exactly these.

With game data (`Registry.load()` with a staged `test/run/built`):

```
Are you sure (Y/N, or R to re-logon)? Y
Have a nice day...
Make your selection (X to exit): M
Make your selection (X to exit): X
Ok, thanks for calling!
Password: *******
Username: Aldric
[MAJORMUD]: E
[MAJORMUD]: X
exp
search
```

Additionally without game data (bare checkout, static patterns only):

```
A fat giant rat creeps into the room from nowhere.
Newhaven, Arena
Newhaven, Narrow Road
The blood-soaked floor is covered in broken, rusted weaponry and some things
The giant rat falls to the ground with a tortured squeak.
The giant rat lunges at you!
The giant rat lunges at you, but you dodge out of the way!
This huge room is has been carved into the earth with crude work- manship.
This narrow road is quite plain save for the various lanterns hanging from
You are about to terminate this connection!
better left unmentioned. A huge iron gate sits in the west wall, and behind it
buildings lie to your north and west.
clash of steel below you, and cries of pain and triumph accompanying it. Stone
in the northern wall.
the trees around, and a large stone stairwell leading downwards. You hear the
you can hear the growling of some unfathomable beast. A large black door is set
```
