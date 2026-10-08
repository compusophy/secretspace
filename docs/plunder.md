# Wandfall: a wand battle royale (game #3, being built)

**Status:** building, on the owner's word (October 2026): "we can leave
luciphon how it is ... archive the legacy. we start a new game from the
new engine, we are cloning the blizzard game plunderstorm". Working title
**Wandfall** (crates `wandfall`, `wandfall-web`; page `/wandfall/`); the
owner may rename it. Luciphon is frozen. The owner's vision, verbatim:

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

## Build stages

### Stage A: the first playable (done, October 2026)
- [x] `crates/wandfall` (engine only): an island from a seed (hills by
      arithmetic-only noise, the shore, 420 trees, 140 rocks, 14 ruined
      rings of pillars, all blocking wizards and bolts); movement (run,
      jump, wade, step up, glide in the drop) in arithmetic only, so the
      page predicts its wizard to the bit (`the_page_predicts_its_wizard_exactly`).
- [x] The match: lobby (12 s once someone waits; warm up unhurt) → the drop
      (everyone glides from 70 m; bots fill to 16) → the fight (a wand bolt:
      75 m/s, 12 damage, 0.37 s cooldown; 100 health, regen after 6 s) with
      a five-phase storm, each circle inside the last → the winner shown →
      lobby. Late arrivals watch, then play the next.
- [x] Bots: see as people do, notice after a moment, lead and miss a little,
      strafe and keep their distance, keep to the next circle.
- [x] The wire (`proto.rs`, fuzzed), the room (`/ws/wandfall`), the page on
      the engine (`render`): first person, the wand in view, bolts of light,
      bursts, the storm's wall, the island map with both circles, the feed,
      names and health over heads, spectating.

### Stage B: spells, levels, loot (in progress, October 2026)
- [x] Levels 1-20 within a match (XP from chests, damage dealt and
      knockouts; more for a higher-level knockout): +8 health and +5% damage
      a level.
- [x] Ten spells as loot, ranks 1-5 (a duplicate ranks one up; each rank
      +12-15% power, 6% less cooldown): offensive (Q, E) Lance, Comet, Chain
      Spark, Starfall; utility (R, F) Root, Blink, Ward, Mend, Gust, Haste.
      Over a full set, hold G to swap out the weaker of that kind.
- [x] 40 chests a match (a third at the ruins), two scrolls each; the
      knocked out drop everything they carried. The lobby hands out a random
      practice set.
- [x] Right click aims down the wand: zoom to 0.72 rad, walk at 55%.
- [x] Bots loot chests and scrolls, and cast: at their mark, Ward when hurt,
      Mend when low, Blink and Haste to outrun the storm.
- [x] The HUD: the spell bar (icons, ranks, cooldowns), level and XP, a ward's
      shield, what lies underfoot; effects for every spell.
- [ ] Touch controls (two sticks, cast buttons).

### Stage C: queues and teams
- A queue screen; matches start at N people or after a wait; duos.
- The hub card (live preview), a results screen, a season of names.
- Netcode at scale (`docs/engine.md` §9): 40-60 people, lag-compensated hits.
