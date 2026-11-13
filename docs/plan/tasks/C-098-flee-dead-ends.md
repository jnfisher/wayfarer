---
id: C-098
title: "Flee runs on, not into a dead end or back along its trail"
tier: B
size: S
wave: 16
depends: []
crate: wayfarer-brain
touches:
  - crates/wayfarer-brain/src/policy.rs (`Candidate::dead_end`, `leads_on`, `flee_exits`)
  - crates/wayfarer-brain/src/flee.rs (the trail)
  - crates/wayfarer-brain/tests/flee.rs
  - docs/brain.md ("Flee")
---

## Goal

A flee with monsters following it keeps running through rooms that lead somewhere, so it can
shake them, instead of running into a dead end where whatever follows corners it.

John (2026-11-12), after Lexi hung up at 8% in the Dark Cave: "this is an opportunity to
double check the fleeing is working as intended." He asked for the fix.

## Inputs

What Lexi's logs on silver-mirror show (2026-11-11, 23:27–23:28). Lexi, a druid, was in a
party with Penny.

- A big bugbear captain and two bugbear berserkers in the Large Cavern hit Lexi for 50–70 a
  round. At 23% she fled e, ne, e, se, e, then s into **Dark Cave, Barracks**. Its only exit
  is the way in. All three bugbears followed, the 6 rooms ran out, and she fought back there.
- At 20% she fled again. The Barracks' only exit is north; from the tunnel junction she went
  back west to the **Large Cavern**, the other dead end. Still followed, she fought back.
- At 18% she fled the same way back to the Barracks, which by now held two ogres and a
  bugbear. She hung up at 8%.
- Flee chose one hop at a time: plain exits first, rooms that spawn nothing first, then the
  direction's order, and not back to the room it had just left. Nothing in that ranking
  noticed a dead end. At the junction (`1/172`, exits s, w, ne, nw) south, the Barracks, came
  first by direction.

## Behaviour

- `flee_exits` marks each candidate `dead_end` when its room does not lead on: fewer than 3
  other rooms within 3 steps of it, by exits Flee could take, not back through the room it
  leaves. The rank is plain exits that lead on (quiet first), then plain exits into a dead
  end (quiet first), then doors and gates in the same order; then not protected; then the
  direction.
- Flee keeps a trail: every room it fled from or through, and every room it was cornered in,
  since HP was last at or above the line. It ranks plain before doors, leading on before dead
  end, off the trail before on it, then quiet before spawning. The trail replaces "not back to
  the room it just left", and lasts across the flees of one stretch under the line, so a
  second flee does not run back toward where the first began.
- A dead end's own exit still leads on. From inside a dead end, Flee leaves by it.
- Unchanged: the flee line, `health.flee_rooms`, the watch for followers, fighting back once
  cornered, and the hang-up line.

## Done when

- `tests/flee.rs` `from_the_dark_cave_junction_the_ways_on_first_and_the_dead_end_barracks_last`:
  from the junction, w and ne come before s and nw (both Barracks). From the tunnel beside the
  Large Cavern, ne comes before the Cavern. From inside the Barracks, its one exit is taken.
- `tests/flee.rs` `followed_out_of_the_junction_it_runs_on_through_the_tunnels_not_into_a_barracks`:
  a fight in the junction under the line flees w, not s. Followed, it goes nw, not back e.
- The existing flee tests pass unchanged.
- fmt, clippy `-D warnings`, nextest.
