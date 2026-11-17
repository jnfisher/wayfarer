---
id: C-108
title: "Afraid, held or blind: the bot waits, or walks on blind"
tier: A
size: S
wave: 16
depends: []
crate: wayfarer-brain
touches:
  - crates/wayfarer-brain/src/executor.rs (`Refused::Afraid`, not `Lost`)
  - crates/wayfarer-brain/src/fight.rs (the fear hold)
  - crates/wayfarer-brain/src/bless.rs (a blessing refused for fear failed)
  - crates/wayfarer-brain/src/modes.rs (the held wait holds walks while attacked; blind, the
    loop goes on; nothing seen clears who is here)
  - crates/wayfarer-brain/src/position.rs ("You are blind." in place of a room is that room)
  - crates/wayfarer-brain/tests/task.rs
  - crates/wayfarer-brain/tests/fight.rs, crates/wayfarer-brain/tests/rules.rs
  - docs/kb/combat.md, docs/kb/changelog.md
---

## Goal

A small terror beast's shriek made Penny afraid for 27 seconds, and she sent "bash beast"
about 7,800 times in that time (John, 2026-11-16). "You are too afraid!" was taken for a line
lost to confusion: sent again at once five times, then Fight attacked again at once, for as
long as the fear lasted. The same day Lexi, engulfed by a big shambling mound ("You can't seem
to move anywhere!") on a Go to that fights nothing, sent `nw` hundreds of times: the moment's
wait after a held move (C-083) was dropped as soon as something attacked her.

Penny, in a nanati shadow's darkbolt ("The room is pitch black - you can't see anything", for
her alone), stood and waited about 100 s while her party fought, and came back up to 10 s after
it lifted. John: "the script isn't performing well when blind in general and I'd like it to
continue on the loop it's on even if blind - try to move, if the move works you should be able
to know the room we're in".

## Behaviour

- "You are too afraid!" is its own refusal (`Refused::Afraid`), not a lost line: the command is
  not sent again by the executor.
- Fight holds every attack and cast for a round (`AFRAID_WAIT`, 5 s) after it, narrating "Fight:
  too afraid to attack, waiting for the fear to pass", then tries once more; each refusal holds
  it again. Flee and the walk are not held (moving is not blocked).
- A blessing refused for fear counts as failed (tried again a round later), not as on.
- Held after a refused move: the wait (`HELD_PAUSE`, 4 s) still lets a fight go on while
  something attacks, but no walk goes before it is over, whoever attacks.
- KB: `combat.md` "Afraid".

- Blind or in the dark, out of combat, Fight wants nothing: a loop or a Go to walks on. A move
  answered "You are blind." counts as arriving, as the darkness line already did, and the map
  says where; nothing seen there clears who and what was "here", so the stop is passed as
  cleared. With nothing else to do it waits and looks every 10 s, every 3 s while something
  attacks it; a room shown ends the blindness. The bot now wakes for that look, and for the
  end of the fear hold.

## Done when

- An attack answered "You are too afraid!" is not tried again for a round, then goes.
- Blind, a loop walks on, stop after stop.
- Blind and attacked, it looks again within a few seconds.
- Held on a Go to that fights nothing, with a monster attacking: the move is not tried again
  for the wait, then goes.
