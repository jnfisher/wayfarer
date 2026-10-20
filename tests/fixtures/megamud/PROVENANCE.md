# Provenance

Nine files of the 2.0 path set (Winterhawk's, public, from mudinfo.net) from MegaMud's
`Default/` directory, for the MegaMud loop tests (card C-036); the rest of MegaMud's data
stays out of the repository. Copied on 2026-10-20, byte for byte, from `MegaMUD_v2.0_Alpha_-_With_Paths.zip` (SHA-256
`9e0bab9dee62ee0f8f46f3ee0f041ae41335a293da240901ac375100a473d21d`) in
`jnfisher/mmud-exploration`'s `reference/megamud/` at commit
`e651a93a4f8c952510475cf89451fdbdc59743a2`. `wayfarer import-world --megamud` reads this
directory as it reads the zip; with no `Default/Rooms.md` the pack has no waypoints.

Do not edit these files (`.gitattributes` keeps their CRLF line ends); a card that needs other
paths adds its own subdirectory.

| File | Why | SHA-256 |
|---|---|---|
| `Default/Mwodloop.mp` | "Wooden Passages", 54 steps, never in the Chieftain's room | `b180d281a3fc2eb1d4d6182aa8e67440ab207b141c6a925a26e5c4a76adfe729` |
| `Default/HKEPLOOP.mp` | "Bandit Keep", inside the courtyard | `be9feb5c2eb07d4da8e537b86593a175094599231040556c43bd8bc8213cfd2d` |
| `Default/Ftrehkep.mp` | into the keep: `n[search n]` at the Hidden Keep, `climb tree` | `4b06b04c02814b00cf8751c30407ea0e2a0c8d9c3f56245ab47083d5d81a8e60` |
| `Default/NARNLOO2.mp` | "Newhaven Arena": the Arena and the Dungeon Entrance | `bfede505f59465f9769a166bd0761eb1b8afda39f9467964030ef25c7d4de227` |
| `Default/NARNLOO3.mp` | "Newhaven Arena (cave bear)": and the Small Cavern | `9885863ff3a719912253831a79f9b9b146938e4033b2655d85d66d8ae46515c6` |
| `Default/Narnloop.mp` | "NewHaven Arena Loop": the Arena only | `571364b1114155562f544761c2309cc9e0d8224e5b899cd982be93b25ccfc502` |
| `Default/Bosploop.mp` | "Oasis Loop": nothing on it to hunt | `45bd1f0ee94f83e85a2a279668d1d0c56f9fe8e2018fb9dcb8a0aea62ba8b37b` |
| `Default/CRSTLOOP.mp` | "Mummy Loop": through a level 10-only exit | `ff2612d7c6631cb34051fbec59ca500ede3a83ce95c7beda85abd9faa213bab9` |
| `Default/VLEFLOOP.mp` | "Giant Efreetis": two hidden exits, `se[search se]` and `nw[search nw]` | `35eeaff6e2cb54cba2247472a3ca04f8434519996c23024bd02167ec120fe7e4` |
