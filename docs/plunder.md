# The next game: a wand battle royale (vision, not yet to build)

**Status:** the owner's vision, recorded October 2026. **Do not build this
yet.** The engine comes first (`docs/engine.md`); this file says what the
engine must one day carry. The owner's words, verbatim:

> "we are making a versions of the blizzard game PLUNDERSTORM ... which are
> short games of a battle royal style -- so there will be queues and
> matches and levels within game (i think u start lat lvl 1 and cap at 20,
> within one match). its a largish map that shrinks over time-- our focus is
> making not classes, but spells or talents trees that allow for massive
> hybridization...ther are like single target seplls, and aoe
> spells....defensive spells/movement spells/healing spells...different cc
> spell,s etc...so theya re rank upgradeable too..maybe iuf we just try to
> make a verison of plunderstorm and just use their stuff for now---
> luciphopn qwill turn into a WAND based first person shoot-er ... and
> possible the more mmo, all tied togerther with both, but the priest class
> only will hve talent tees, light and shadow or hybrid are the choices
> basically and healer and dps for mulktipeolrayer team vs team based
> play...in the futyure. but for exactly, right click should aim down wand
> instead of charging an attack?"

## What it is
- **A first-person wand shooter.** Short battle royale matches with queues
  and matchmaking. Everyone starts at level 1 and can reach 20 within one
  match; levels reset each match.
- **A large map that shrinks.** A storm wall closes in over time.
- **No classes. Spells found and upgraded in the match:**
  - Kinds: single-target, AoE, defensive, movement, healing, and crowd
    control (stun, root, slow, silence, knockback).
  - Each spell ranks up, and spells combine into hybrid builds.
- **Inspiration: Blizzard's Plunderstorm.** The owner suggests mirroring its
  structure first: spell and upgrade drops, a few ability slots, levelling
  from kills and chests, a shrinking storm, solo and duos. Use it as a
  structural guide only; our own names, art and numbers.
- **The future:**
  - The MMO world may tie in.
  - One class, the priest, with talent trees: Light, Shadow, or a hybrid.
  - Roles: healer and DPS, for team-vs-team play.

## What it asks of the engine (`docs/engine.md`)
- **Maps:** a kilometre-scale map with streaming and LOD (Phase 7).
- **The storm wall:** volumetric, readable from both sides (Phase 5).
- **Heavy spell VFX:** hundreds of simultaneous projectiles, AoE fields and
  shields, each lit and readable (Phases 3 and 5).
- **Aiming down the wand (right click):** zoom, a tighter aim, depth of
  field, and a first-person viewmodel with its own FOV (Phases 1-2).
- **Netcode:** 60 players a match at 60 Hz, lag-compensated hitscan and
  projectiles, queues and match rooms (§9).
- **Readability:** teams and enemies distinct at range, without breaking
  the art.

## Open questions (for later)
1. Match size and team modes: solo, duos, trios? [Solo and duos first, 40-60 players.]
2. Match length: [10-15 minutes.]
3. Ability slots and the spell pool size for v1: [4 slots; ~16 spells, 3 ranks each.]
4. Right click: aim down the wand, or a charged attack? The owner leans to
   aiming down. [Aim down: zoom, tighter aim, slower movement.]
5. How it ties to Luciphon's persistent world, if at all: [Separate at first;
   cosmetics carried between them later.]
