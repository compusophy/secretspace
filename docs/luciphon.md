# Luciphon: the spec for game #2

**Status:** this is the plan for game #2 and the checklist for its first two milestones. It is written for an engineer who knows the repo (read `CLAUDE.md` first) and nothing else.

**Revised: first person, in 3D.** After Stage 2 the owner found the one-thumb, top-down movement wrong for the game and asked for full 3D in first person, with standard FPS controls, keeping the world. §3 (camera and look), §4 (controls) and §7's movement now describe that, and the checklist "First person" (§15, after Stage 2) records it. Everything else stands: the island, fights, gathering, hearths, land, residents and saves. Drift, slingshots, spin-outs, the skid strike, teeter and the ledge save are gone; where later sections still name them, read them as history.

**Where it comes from.** Concept #1 (the Lumen) is the core: its movement, its combat, and its rule that nothing else gets built until running and knocking bots around is fun. Concept #4 supplies the plumbing: object-safe room hooks, a soul id that will need no migration later, fixed tile sizes, and fallback glyphs for cosmetics. Concept #3 supplies the meaning: the Underlight, Depth, the Daimon, witnesses, the Ouroboros Pool, and the rule that every layer has three roles. Concept #2 supplies the world's memory: genesis, Strata, the Tithe, and land that remembers its shape.

**How to read the numbers.** Every number here is a starting value. World rules live in `crates/luciphon/src/laws.rs` as `Laws`, which the server and the page share. Gesture thresholds, camera and effects live in `crates/luciphon-web/src/laws.rs` as `Feel`, which only the page uses, so tuning the feel never restarts the world (rule 3). Lines marked **Decided:** settle something the owner left open, and a one-line reason follows each. Lines marked **Proposed:** are waiting on an open question (§18).

---

## 1. Pitch

Luciphon is a persistent island of light floating in the Dark, and you play it in first person. You are a **Lumen**, a small hooded light-bearer.

- **Moving and fighting.** Run where you push, jump gaps, dash through a blow. Strike whatever you look at: a birch gives wood, a rock gives stone, a rival gets launched. The dimmer someone is, the further they fly, so kills come from knocking people into thorns, into walls, and off the Rim where the island ends.
- **Between fights.** You punch trees on their ring. The **glim** you carry orbits you as visible motes. It is your money, your night sight and your ammunition, and it also makes you easy to spot. You light a hearth, paint your land with light by running loops out from it, and plant sunwheat on it.
- **At night.** The Hush rises, and only lit ground is safe. A Wellspring erupts somewhere in the Dim for whoever dares to go and take it.
- **When you fall.** You pass through the **Underlight**: the same world in negative, with your path since your last return drawn as one thread of light and your killer marked. Then you come back.
- **The world remembers you.** It keeps your hearth, your land, and the resident who has held a grudge since you threw him into your thorns.
- **Restarts and watchers.** A server deploy is the world holding still; nobody is ever sent back to a menu. Later, people watching you show up as stars in your sky, and the other secretspace games run inside this world.

**What a player tells a friend:** "It's Smash on one thumb, on a floating island in the dark. You punch trees, light up your own land and knock people off the edge, and the world is still there tomorrow."

## 2. Pillars

1. **Movement is the skill.** Running, jumping, the dash, the wall-kick, and never walking off the edge. How fast you arrive decides how hard you hit.
2. **Light is everything.** Your Flame is your life. Carried glim is your wallet, your sight and your ammunition. Your land is ground you lit. Carrying light makes you seen.
3. **One grammar, two hands.** Standard first-person controls on a desktop (mouse look, WASD) and twin thumbs on a phone, with the same verbs underneath: strike, dash, jump, charge, release. The server holds every Intent to that grammar (`thumb`), a person's or a resident's.
4. **The world remembers, and never kicks you.** From v0.2 the world is saved every 10 s. Leaving is a safe Dream, a deploy is the Stillness, and you come back where you stood.
5. **Layers within layers.** In every layer one lives, one witnesses, and the witness can send a little light down. That covers the hub, the world, the Underlight, the Pool and the desk.

## 3. Camera and look

**Decided (revised): first person, drawn in 3D by the GPU.** The camera is your Lumen's eyes, 1.05 tiles up; the mouse or the right thumb turns it at once, between ticks. The world is low-poly and lit: "lantern noir" in three dimensions.

### The renderer

- **WebGL2 driven from Rust** (`kit::gl`, GLSL in Rust strings; still no hand-written script). Rule 1 of `CLAUDE.md` allows it for Luciphon only; wyrm and the hub still draw every pixel with `pixels`.
- **A mesh a chunk** (`luciphon-web/src/scene/chunk.rs`): the ground (each tile its kind's colour, land tinted by its claim's hue, natural ground slightly uneven), cliffs where the island meets the Dark, a rocky underside hanging deeper toward the middle with stalactites, and every object standing on it (`scene/things.rs`). A chunk is rebuilt when a tile in it changes, at most two a frame, nearest first. The whole island is about 320k vertices.
- **Moving things** are drawn one by one: hooded Lumens in their soul's hue with their Flame's light in the hood, motes, glim on the ground, after-images of a dash, a stunned flicker, a ghost's transparency.
- **Light.** A dim moon and sky, and the sixteen lights nearest the eye: the Luciphon, lanterns, hearths, crystals, glowmoss, every Lumen, mote and glim, and your own. Fog closes in from 9 to 27 tiles; glowing things show through it.
- **The sky**: a gradient with stars above, the Dark below with the Underlight's faint violet far down.
- **Your hand**: a wisp of your light low on the right, drawn back in a wind-up, across in a strike, swelling while you charge (gold in the perfect window).
- **The HUD** is a `pixels` layer over the picture, shown sharp: the crosshair and a charge filling round it, Flame and breath, names and Flame over Lumens near you, markers toward the Luciphon, your hearth and Beacons (held at the screen's edge when off it), what you carry, the phone's stick and buttons, and a red flash when you are hit.
- **Sight.** The server sends what is within 24 tiles (§14). The fog hides the edge of it.

### Three cameras, three meanings (§11)

- **Watched:** your own eyes.
- **Unwatched:** the Underlight, a camera high over where you fell, looking down at the same world in negative, with your path as a thread of light.
- **Watching (later):** a framed screen inside the world for nested layers.

### Look: "lantern noir"

- **Palette.** The Dark `#0b0d1a`, light gold `#ffd27a`, ember `#ff7a3d`, kin teal `#5fd3c8`, Hushling violet-grey `#7d6f93`, Sanctum marble `#e9e4d8`, Rim glass `#7fe0ff`. Each soul has one hue, which tints its land and its robe.
- **Feel, drawn only.** Sparks that burst and fall, rings that spread on the ground, a shake and a flash when you are hit, the strike's reach drawn as a fan, the build ghost as a glowing box, and the rings of nodes near you (strike on the ring for double).
- **Sound (v0.3, `kit::Sound`, synthesized in Rust).** Everything is a bell, tuned to one pentatonic scale; resonant strikes are notes; the Luciphon tolls the hours; the Hush is real silence.

## 4. Controls

**The wand (owner, revised again): no melee.** A bolt of light flies 15 tiles the moment you fire, stopped by anything solid, and takes the first body within half a tile of its line (7 damage, a light shove); one every 8 ticks. A node it meets within 3 tiles gives, as a strike did. A great beam (a charge let go, the old heavy's numbers and perfect window) and a lance (a bolt out of a dash) pierce everyone on their line. The page draws your own beams the moment you fire, from your wand's tip.

**Decided (revised): standard first-person controls.** Direct and responsive: you go where you push at once and stop at once, with no drifting or skidding.

| | Desktop | Phone |
|---|---|---|
| Look | The mouse, once the page has it (click to play; Esc gives it back) | Drag with the right thumb |
| Move | WASD or the arrows, relative to where you look | The left thumb: a stick wherever it lands (walk inside 44 px, run beyond) |
| Wand: a bolt | Click | Tap with the right thumb |
| Wand: a great beam | Hold the click (from 220 ms), let go | Hold the right thumb still (from 320 ms), let go |
| Throw | Hold the right button, let go; look up to throw further | The throw button, held and let go |
| Sprint | Hold shift (spends breath) | Push the stick to its edge |
| Jump | Space | The jump button |
| Dash | Q (the way you move, or the way you look standing still) | The dash button |
| Heart | E opens the wheel, 1-8 picks; B build, K kindle, R rekindle, T recall, H chirp | The heart button opens the wheel; tap a slot |
| Build | B, then 1-6 for a piece; click places, hold removes; B again is done | The wheel's build, then a piece; tap places, hold removes |

- Every Input carries where you look (`aim`); strikes, charges and throws go there, and your body faces it.
- **The grammar** (`thumb.rs`): a charge begins only when none is under way, and only a charge can be released or cancelled; strikes and dashes may come while moving. The server trims anything else, so a script gains nothing over a person.
- **Thresholds** live in `Feel` (`luciphon-web/src/laws.rs`); the input logic is `controls.rs`, pure and tested natively.
- **Hints.** One line at a time until you have done each once (move, strike, jump, dash, charge), stored in `secretspace/luciphon/taught3d`.

### Where depth comes from

| Technique | Input | Result |
|---|---|---|
| Launcher | Third bolt landed on one target within 1.2 s | Double knockback |
| Lance | Fire during a dash, or just after | A piercing bolt: 10 damage, 1.3x knockback |
| Dash cancel | Dash during the wand's recovery | Recovery skipped |
| Sprint | Hold shift | 8.5 tiles/s for about 4.5 s of breath; run dry and you are winded until it is back to 35 |
| Wall-kick | Dash into a wall, then dash within 10 ticks | A free rebound dash |
| Jump | Over a gap, or over a low blow | You keep a third of your control in the air |
| Feint | Start a charge, read their dash, let go early | Nothing spent; positional mind games |
| Perfect release | Release a charge 0.60-0.73 s after the press | A 34-damage great beam, or a piercing throw |
| Resonant gather | Strike a node on its ring | Double yield; every third in a row rings the node out |
| Flow 1-3 | Chain techniques within 1.5 s of each other | +15% knockback, a full charge, and you become a beacon |

**Casual floor.** Dwelling beside a node after one strike keeps striking it at the base rate. **The server never fights players for you.** Timing windows belong to the server and outcomes are positional.

## 5. Core loops

- **Seconds: read, move, commit.**
  - Drift around a birch and straighten as the gold spark shows.
  - Dash through a thrown mote during its i-frames.
  - Lift and tap a skid strike. Your 6 tiles/s plus a rival running at you at 2 is 8 tiles/s of closing speed, which deals 14 damage. At 40 Flame the target flies 2.2x as far, into brambles and back into your follow-up.
  - Or quieter: tap a birch on its ring three times in a row, and it rings out.
  - Every second asks the same questions: stop or move, tap or aim, carry or bank.
- **Minutes: venture, carry, return.**
  - Fill your bag with wood and stone (every 50 carried slows you 5%) and gather glim, which orbits you and makes you seen.
  - Choose your route home: the ice pond is fast but open, the woods are slow but safe.
  - Bank at your hearth, where banked goods are never at risk.
  - Build walls, thorns and lanterns, plant sunwheat, and run a kindle loop to light new ground before someone snuffs your open wick.
- **A session: one or two 15-minute days.** Ten minutes of day, one of dusk, three of night, one of dawn.
  - At dusk the Luciphon tolls and marks tonight's Wellspring.
  - At night, Hushlings crawl up from the Dark and only lit ground is safe. The Wellspring pours glim for 90 s to whoever stands in it, which is the fight everyone converges on.
  - At dawn the nodes renew.
  - You leave by lifting off out of combat: an instant, safe Dream that builds rest while you are away.
- **Months: meaning that stays.**
  - Seven skills to 99, each with an all-time hiscore.
  - Depth (your costly returns) shown as halo rings, and records such as longest drift chain and fastest Rim lap.
  - Land that remembers its shape, kin, the Codex, and cosmetics that travel to wyrm and every later game.
  - *If open question 1 is accepted,* an **Age** turns every 8 weeks. The outer world and the gear renew, the Age's champions are carved into the Luciphon, and the old holds stay as named ruins (Strata).

## 6. The world

### v0.1: the Vale

- One room, `luciphon`, at 30 Hz.
- A 128x128-tile grid in 16 chunks of 32x32 tiles, holding one island: a disc of radius 60 tiles (about 11,300 tiles) with a ragged Rim (value noise of plus or minus 3 tiles), floating in the Dark.
- It is generated from `WORLD_SEED` with `engine::rng`. The seed is stored in the snapshot.

| Ring | Radius (tiles) | Ground and content | Rules |
|---|---|---|---|
| **Sanctum** | 0-10 | Marble. The **Luciphon**, a 3x3 bell-lantern at (0,0), always lit (light radius 10). Four Keeper pillars at r 7 (solid, good for wall-kicks). Three teaching birches at r 9. The spawn point. | Strikes only shove: no damage, no wall slam, base knockback. You cannot gutter here. No hazards. |
| **Glow** | 10-34 | Meadow and moss. Birch groves (4% of tiles), rocks (2%), glow-moss (1.5%). No void, no brambles, no ice. | Knockdown only (§7). Most hearths go here. |
| **Dim** | 34-52 | Darker moss. Oak and birch (6%), rocks (3%), moss (1%). Three ice ponds (radius 3-5), two mud fields, one shallow stream, four chasms (void blobs of radius 2-4), six bramble patches. Three **Wellspring** sites at r 44, 120 degrees apart, each at least 6 tiles from any chasm. | Open play. Gutter at 0 Flame, dropping 50% of what you carry. Yields x1.6. |
| **Rim shelf** | 52 to the edge | Glass ground, crystal nodes (3%), rocks. Eight Rim checkpoints at r 56, one every 45 degrees. A lap passes within 3 tiles of all eight, in order, in either direction. The edge drops into the Dark. | Open play. Drop 100% of what you carry. Yields x2.5. Being knocked off the edge is death. |

**Decided: Rim edges and chasms exist only in the outer two rings.** Why: newcomers in the Glow cannot be bumped to their death.

**Capacity.**

- Hearths fit between radius 16 and 48 at 10-tile spacing: about 60 of them, 10 of which are residents'.
- When no spot is free, a newcomer gets a **lodging** at the Sanctum's edge (a vault and a respawn point, but no land) until a ruin frees a spot.
- Region growth (v0.4) triggers at 80% occupancy.

**Day and night (v0.2; in v0.1 it is always day).** The cycle is 15 minutes and deterministic from the world tick.

| Phase | Minutes | What happens |
|---|---|---|
| Day | 0-10 | Sight 10 tiles. |
| Dusk | 10-11 | Sight eases to 6. The bell tolls. Tonight's Wellspring is marked at the screen edge. |
| Night | 11-14 | Sight `6 + 0.2 × sqrt(glim)`, capped at 9. Hushlings rise. The Wellspring erupts from 11:00 to 12:30. |
| Dawn | 14-15 | Hushlings dissolve into 1 glim each. Every depleted node renews. |

The Luciphon pulses on every in-game hour (every 37.5 s).

### Creatures and flashpoints (v0.2)

**Lit tiles.** A tile is lit if it belongs to a *fed* claim (one whose vault holds glim), or lies inside the light of a lantern, a hearth or the Luciphon. An unfed claim's tiles and lanterns are unlit.

**Hushlings.**

- Night only. They spawn on unlit tiles beside the Rim edge and the chasms, at most 24 at once.
- Flame 20, speed 4 tiles/s. They bite for 6 damage after a visible 300 ms tell, and take 1.5x knockback.
- They are drawn to carried glim within 12 tiles (more glim pulls harder) and to unfed claims, whose lanterns they snuff. A snuffed lantern relights when its vault is fed again.
- They never enter lit tiles.
- They drop 3 glim when killed.

**The Wellspring.** One a night, at one of the three Dim sites (which one is a seeded function of the day number). It is a column of light of radius 2.5 tiles that pours 4 glim/s, shared equally among every awake Lumen standing inside, residents included. It pulls a sparse server together.

### How it never feels empty

- **Wanderers.** 16 persistent residents (§10, Stage 3.4). CROWD is 16 awake Lumens when it is quiet, with MIN_BOTS 4 always awake. As people arrive, residents walk home and Dream rather than vanishing; when it is quiet they wake.
- **Genesis.** Whenever the room boots with no snapshot, or with a snapshot that has no RESIDENTS section, the server runs two in-game days (54,000 ticks, about 6-10 s) headless with residents only, takes a snapshot, then opens. The first human finds hearths lit, walls up and loops kindled: the world was old before you came.
- **Flashpoints, the island's small size, and dawn renewal** keep people meeting.

### Later

- **Region (v0.4).** At 80% hearth occupancy the island grows to radius 96 with an **Umbra** ring (Glow 10-34, Dim 34-60, Umbra 60-86, Rim 86-96), and only dirty chunks are persisted.
- **Camps (v0.6).** Gloamwolf dens in the Dim: packs of 3 that circle and lunge after a 400 ms tell, back 5 minutes after the den is cleared.
- **Boss (v0.6).** **The Unsung** rises at the Rim every 2 hours, on a public timer shown at the screen edge. Inside its silence chirps fail and nodes stop ringing. It is hurt in proportion to how many *different* souls are Calling at once, so strangers must make noise together.
- **Zones (v1.0).** Interiors, caves and Nooks are zones inside the same room, entered without reconnecting. The chunk message reserves a `zone` byte now.
- **Instances (v0.9).** When one world fills, a sticky whole-world copy opens (`/ws/luciphon/<n>`). Never per-area phasing.

## 7. Movement and combat

### Units and determinism

- **Positions and velocities are fixed-point Q16.16 in tiles** (`engine::fixed::Fx`).
  - Velocities are tiles per tick.
  - Headings are `u16` (65536 to a turn).
  - `sin` and `cos` come from a **committed 1024-entry `i16` table** with integer interpolation.
  - `atan2` and length (an `isqrt` on `i64`) are integer too.
- `Fx` has only saturating operations, and no bare operators, so debug tests and release builds behave the same.
- **No `f32` or `f64` in anything prediction touches:** motion, terrain, collision, timers, charge, teeter.
- Timers are in ticks. At 30 Hz a tick is 33.3 ms.
- Laws are written in readable units through integer `const fn`s, for example `run: tps(6_000)` for milli-tiles per second.
- The result is bit-identical motion in wasm and native. The test `prediction_matches_the_server` enforces it.

### Movement

| Law | Value |
|---|---|
| Walk / run / sprint top speed | 3.0 / 5.5 / 8.5 tiles/s (the stick's push sets a walk's share; a sprint spends 22 breath a second) |
| Acceleration / stopping | 45 tiles/s² each: up to a run in 4 ticks, stopped in 4 |
| Air control | 30% of that |
| Jump | 6.5 tiles/s up against 20 tiles/s² of gravity: about a tile high, 0.65 s in the air |
| Striking / charging | Top speed ×0.7 / ×0.5 |
| Body radius | 0.35 tiles. Bodies push each other apart softly after the step (not predicted). |
| Weight | Top speed × (1 - 0.05 × floor(materials/50)), never below 0.7. Glim weighs nothing. |
| Own kindled ground | Top speed ×1.15 |
| Ice / mud / shallow water | Ice: acceleration ×0.15. Mud: top speed ×0.5. Water: top speed ×0.6, no dash. |

**The step.** `motion::step(&mut Body, &Intent, &Tiles, &Laws)` runs in this order every tick:

1. Count down the timers and regenerate breath.
2. Read the ground under the body for its multipliers.
3. **Hit-stun:** no control, and flight speed decays at 12 tiles/s² (×0.15 on ice).
4. **Dashing:** velocity is the dash velocity.
5. **Control:** facing becomes where you look. The wanted velocity is the stick's heading at its top speed (throttle × weight × own land × ground × striking or charging); the velocity moves toward it by the acceleration (or the stopping rate with the stick up), a third as fast in the air. A jump, on the ground, starts the climb.
6. **Move** by the velocity, colliding with solid tiles one axis at a time (x, then y). A contact may cause a wall slam or open the wall-kick window.
7. **Height:** gravity while in the air; landing on ground; over the void with nothing under you, you fall, and 8 tiles down the Dark takes you (cause: the Dark). A jump carries you over a one-tile gap.

**Dash.**

- 3 tiles over 5 ticks (18 tiles/s), with i-frames on ticks 0-2, the way you move (or the way you look, standing still).
- Costs 30 of 100 breath, with a 0.35 s cooldown after the dash ends. Afterwards, speed is the lower of your prior speed and run speed, along the dash heading.
- A dash cancels strike recovery. A dash over the void ends in a fall.
- A dash pressed in a dash's last 3 ticks is held for a wall-kick.

**Breath.** 100. It regenerates at 35/s once 0.4 s have passed since you last spent it.

**Wall-kick.** A dash into a solid tile stops you. A dash within 10 ticks (333 ms), or one held from the dash's last 3 ticks, is a free rebound: no breath, once per wall contact.

**Flow 1-3.**

- These count as techniques: wall-kicks, resonant hits, landed strikes, lances, perfect releases, and dodges (an i-frame passing through a hit).
- Each one within 1.5 s of the last raises Flow by 1, up to 3.
- At Flow 3 you deal +15% knockback, your next charge starts full, and you are a beacon visible from 2x sight.
- Flow falls by 1 after every 1.5 s without a technique.

### Flame, damage and knockback

- **Flame** is your life: 100. It regenerates 3/s after 6 s without damage, or 6/s on your own claim.
- **Knockback multiplier** on the target: `KB = 1 + 2 × (100 - flame)/100`, applied after the hit's damage. At 25 Flame you fly 2.5x as far.
- **Flight and stun.** A hit sets the target's velocity to the knockback. Hit-stun lasts `4 + floor(kb_speed / 2)` ticks, with kb_speed in tiles/s. There is no control while the flight decays (step 3).
- **No random spread or random crits, ever.** Behaviour is deterministic and learnable, and it keeps the mirror exact.

**Targeting.**

- A strike reaches 1.2 tiles in a 100-degree cone in front of you, and facing turns up to 50 degrees toward its target.
- It picks, in this order:
  1. a Lumen *engaged* with you (either of you struck the other in the last 10 s);
  2. a creature;
  3. a node, or from v0.4 one of your own damaged pieces;
  4. any other Lumen you may damage, but only if it is in the inner 40 degrees of the cone.

| Attack | Timing | Reach and shape | Damage | Knockback |
|---|---|---|---|---|
| **Strike** (tap) | Wind-up 4 ticks (the fist visibly gathers light), active 2, recovery 6 | The cone | `8 + 0.75 × closing speed` (tiles/s), capped at 16 | `(3.5 tiles/s + 0.5 × your velocity along the strike) × KB` |
| **Launcher** | Third strike landed on the same target within 36 ticks | As strike | As strike | ×2 |
| **Lance** | Tap during a dash, or up to 2 ticks after it (judged by input seq) | A line 1.8 tiles ahead | 14 | ×1.5. A whiff costs 12 ticks of recovery. |
| **Heavy** (hold, short or no aim) | The charge c counts in ticks from the press; release at c 12-36. Lunge 1.5 tiles over 4 ticks, then 10 ticks of recovery. | First valid target on the lunge path, within 1.2 tiles | 12 at c 12, rising linearly to 28 at c 18 (full). **Perfect** (c 18-22, 0.60-0.73 s): 34. | 6-10 tiles/s × KB. Perfect: ×1.5. |
| **Throw** (hold, aim of 40 px or more) | Same charge. Needs **3 carried glim**, spent on release. Without them the arrow draws grey and the release is a heavy along the aim. | A mote at 14 tiles/s. Range 4-9 tiles by arrow length (40-160 px). Stopped by walls, trees and rocks. | 6 to 14 by c. **Perfect:** 20, and it **pierces**. | 3 tiles/s × KB. Perfect: ×1.5. |

- **Skid strike.** For 10 ticks after a slow lift, a strike counts your speed at the lift, for both damage and knockback.
- **Overcharge.** Past c 36 (1.2 s) the release is a plain strike.
- **Interrupts.** A hit of 8 or more damage while you charge ends the charge. Stillness is the price of aim.
- **A thrown mote that misses lands as a 3-glim pickup anyone can take.** One that hits is lost to the Dark, which is a sink.

**Hazards.**

- **Thorns** (natural brambles; built thorns from v0.2): 12 damage plus a bounce of 8 tiles/s along the thorn's outward normal, with 0.5 s of contact immunity per thorn piece.
- **Wall slam:** hitting a solid tile faster than 7 tiles/s deals `(v - 7) × 4`. Never in the Sanctum.
- **Void:** fall, as above.

### Death: the Descent, and the return

**Guttering** (0 Flame in the Dim or on the Rim, or a fall):

1. Your carried goods drop by ring (50% in the Dim, 100% on the Rim) and scatter as pickups within 1.5 tiles. **Half of every glim drop is taken by the Dark** (a sink).
2. Then **the Descent**, which *is* the 3 s respawn wait, so it adds no time:
   - **Dissolution:** a 0.3 s white overexposure.
   - **The Underlight:** 2.7 s of the last frame in negative, inverted once and then reused, with your body gone. Your path since your last return (recorded by the page, the last 5 minutes at most) is drawn as one thread of light, and your killer is marked.
   - Two lights in the Underlight, your hearth and the Luciphon, are your respawn choice. Tap one. The hearth is the default; until you have one, the Luciphon.
3. **Return.**
   - **Ghost** for 90 ticks (3 s): you cannot be harmed or harm anyone. It ends early if you strike a Lumen.
   - **Rekindled:** +15% breath regeneration for 60 s for each *costly* death in the last 10 minutes, up to 3 stacks (Counter-Strike's loss bonus).

**Knockdown in the Glow.**

- At 0 Flame you are down for 2 s: you can be shoved but not damaged.
- You then stand where you are at 30 Flame, with 5 s of ghost. No drop, no Descent.
- The same attacker cannot damage you in the Glow for the next 60 s.

### Newcomer safety

- **Sparks (from v0.2; switched off in v0.1 by the `sparks` law).** These last for your first 30 minutes of play.
  - Outside the Sanctum and the Rim, Lumens deal you no damage and no knockback.
  - In the Sanctum, shoves apply to and from you as to anyone.
  - On the Rim you are fair game.
  - A small spark floats over your head.
  - Striking a Lumen outside the Sanctum ends your Sparks, and that strike lands normally.
  - Residents count as Lumens for this rule.
- **Residents never start a fight with a Spark.** Both rules use one duration, measured on the target's play time.

### Netcode

- **Rates.** The server ticks at 30 Hz and sends a frame every tick. The page draws at most 60 fps.
- **Input contract.**
  - One 9-byte `Input` a tick (30/s, well under RATE 90), with at most one verb each. A second verb waits for the next Input.
  - Every server tick consumes **exactly one** queued Input.
  - An empty queue repeats the last stick with no verb, counted in the Frame's `repeats`.
  - The server never merges sticks. Past 4 queued Inputs it drops the oldest and flags that in `queue`.
  - The page nudges its send clock by up to 1 ms a frame to keep the queue at 1-2.
- **Own movement is predicted.**
  - On each frame the page resets its body to the authoritative state at `ack` and replays its unacknowledged Inputs.
  - Errors under 0.25 tile are blended over 100 ms; larger ones snap.
  - Predicted: motion against static tiles, jumps and falls, dash, and the drawing of your own wind-ups and charges.
  - Not predicted: hits, damage, knockback you receive, body collisions, pickups.
- **Two timelines.**
  - Everything deterministic (your body, node rings, the clock) is drawn in **predicted time**.
  - Other bodies are interpolated 2 ticks (66 ms) behind the newest frame, using wyrm's smoothed alpha.
  - So timing windows are fair without trusting the browser:
    - **resonance** is judged on the server's tick, which is the tick the page drew the ring on;
    - **charge length** is `release seq - (hold seq - held_for)`, which latency does not change. The server checks it is consistent with arrival times within 6 ticks;
    - the **wall-kick** and the **lance** use their Input's own seq.
- **Lag compensation** for strikes, heavies and lances by Lumens.
  - Target bodies are rewound by `min(rtt/2 + 66 ms, 100 ms)`, at most 3 ticks.
  - The rewind uses a 12-tick history ring, which residents' perception shares.
  - RTT comes from the page's Ping.
  - Throws are simulated on the server from the thrower's current position, and the page predicts its own mote visually.
- **No parry.** At phone latency it would be a coin flip.

## 8. Gathering, building, farming, territory (v0.2)

### Gathering

| Node | Where | Gives per strike | Strikes | Regrows |
|---|---|---|---|---|
| Birch | Sanctum edge, Glow, Dim | 3 wood | 8 | 4 min |
| Oak | Dim | 4 wood | 10 | 6 min |
| Rock | Everywhere outside the Sanctum | 2 stone | 8 | 5 min |
| Glow-moss | Glow, Dim | 1 glim | 3 | 3 min |
| Crystal | Rim | 2 glim | 4 | 8 min |

- **Ring multiplier.** Glow ×1, Dim ×1.6, Rim ×2.5. Fractions accumulate per player in milli-units, so nothing is lost to rounding.
- A node's strikes are shared: whoever strikes it uses them up.
- **Dawn.** Every depleted node renews.
- **Resonance.**
  - Each node rings every 30 ticks on a seeded phase (`hash(seed, tile) % 30`), drawn as a ring pulse.
  - A strike whose active frame starts within 2 ticks of a ring is **resonant**: double yield, a chime flash, a haptic tick.
  - **Every third resonant hit in a row rings the node out:** +3 wood or stone, or +1 glim, and once depleted the node regrows 25% faster.
  - A birch struck resonantly every time gives 54 wood, against 24 without resonance.
- **Dwell.** After you strike a node, staying planted within reach keeps striking it every 30 ticks at base yield, never resonant. Any input stops it. Dwell works only on nodes and, from v0.4, on repairs of your own pieces.

**Carrying.**

- Materials (wood, stone and sunwheat together) up to 300; glim up to 500. Anything over the cap stays on the ground.
- Pickups are collected by walking over them.
- Carried glim is shown as orbiting motes (1 per 10 glim up to 12, after which the motes grow).
- **Beacons.** Carrying more than 150 glim makes you a **Beacon**:
  - you are marked at the screen edge for everyone within 24 tiles, day or night;
  - at night you are visible from 2x your light radius (`0.2 × sqrt(glim)` tiles) plus 2 tiles.

**Banking.**

- Standing on your hearth's core pours everything you carry into the vault over 1 s, except your **lamp**: you always keep 30 glim, topped up from the vault.
- The vault has no cap and is **never raidable**.
- Building inside your claim pays from your carried goods first, then from the vault.

### Building

**Decided: structures cannot be harmed by other players before v0.4.** Why: it removes griefing and raid balance from the first playable.

| Piece | Cost | HP (from v0.4) | Effect |
|---|---|---|---|
| **Hearth** | 30 wood, 20 stone, 10 glim | Indestructible | A 3x3 object at the centre of a 5x5 **core**, which is your first claim. Vault, respawn point, light radius 6. One per soul. |
| Wall | 4 wood | 120 | Solid to everyone, the owner included |
| Door | 6 wood | 120 | Passable only for the owner (and kin, from v0.4) |
| Thorns | 3 wood, 1 stone | 80 | 12 damage plus an 8 tiles/s bounce to anyone but the owner |
| Lantern | 2 glim (needs Kindling 5) | 40 | Light radius 4. It lights its tiles and extends night sight for everyone near. |
| Planter | 4 wood | 40 | Holds one crop on a claimed, lit tile |

- **Hearth placement.** Only in the Glow or the Dim, at radius 16-48, and at least 10 tiles centre to centre from any other hearth.
- **No walling-in.**
  - Nothing is built or kindled inside radius 13.
  - No piece goes within 2 tiles of another soul's claim.
  - A placement that would cut off a foreign hearth's core from the Sanctum over walkable tiles is refused. Other people's doors count as solid, and the check is a BFS over 128x128 tiles that takes microseconds.
- **Other pieces** may be placed only on your own claim, and never on the hearth itself.
- **Build mode.**
  1. Open the wheel, choose N (Build), then pick a piece. A ghost sits on the tile in front of you, within 2 tiles; steering moves it.
  2. A tap places the piece on the ghost tile after a 12-tick (0.4 s) channel that moving cancels.
  3. Placement repeats while you have materials. Tapping the Heart, or 8 s idle, ends build mode.
  4. A hold with the ghost on one of your own pieces removes it for a 50% refund.
  - Building is deliberate and never a combat speed-build (the Zero Build lesson).
- **Removing a hearth.** A hearth cannot be moved. Removing it returns its land to the commons (the outlines stay), refunds half its cost, and moves its vault to a lodging.

### Farming

- **v0.2: Sunwheat.**
  - Planted on a Planter. It grows only on lit tiles, offline too, and is ripe in 15 minutes.
  - A ripe crop holds full value for 30 minutes, then loses half.
  - Harvest it with a strike.
  - Carried Sunwheat feeds Rekindle in place of 10 glim.
  - An unfed claim's crops stop growing.
- **v0.4:** **Glowberry** (40 min: 6 glim and a 25-Flame heal) and **Lanternroot** (2 h: a dye base for cosmetics). Farms produce while you are away fighting: the macro game behind the micro game.

### Territory: kindling

- **Your claim** is your core plus every tile you have kindled. It carries your hue as a faint light and a pulsing border.
- **Kindle mode** (wheel E).
  - While it is on and you are on commons tiles within 16 tiles of your hearth, every tile you enter becomes your **wick**: open, glowing, tinted.
  - Each wick tile stakes 1 glim from what you carry. With no glim, the wick does not extend.
  - The wick is at most 48 tiles long; at 48 it blinks and stops.
  - A wick that stays open for 60 s gutters, and its stake is lost.
  - Crossing your own wick is harmless.
- **Closing a loop.** Step back onto your claim with an open wick.
  - The commons tiles enclosed between your claim and the wick become yours. They are found by a flood fill from the border of the wick's bounding box, at most 64x64. Each costs 1 glim, from what you carry, then from the vault.
  - If the enclosure would exceed 300 tiles or take you past your claim cap, only the wick is claimed, and the wick only up to the cap.
  - Other claims and radius 13 act as boundaries. A loop that would enclose a foreign claim claims only its wick.
- **Snuffing.** A creature, or a Lumen who may damage you there, touching any of your open wick tiles snuffs it:
  - the wick vanishes and its stake is lost;
  - you take 20 damage and a 0.5 s stun;
  - you are marked at the screen edge for 5 s.
  - It is greed against risk, never an instant kill.
- **Claim cap.** `64 + 32 per hour of active play` kindled tiles, up to 600, earned per soul, so a new name cannot grab the map.
- **What kindled tiles give.**
  - +15% speed for you.
  - Safety from Hushlings while the claim is fed.
  - **0.25 glim per tile per hour** paid into your vault, at half rate while you Dream.
- **Upkeep.** `0.03 × n^1.4` glim an hour from the vault for n kindled tiles, charged every 60 s and halved while you Dream. **The core neither earns nor costs.**

  | Kindled tiles | Upkeep per hour | Yield per hour | Net per hour |
  |---|---|---|---|
  | 50 | 7 | 12.5 | +5 |
  | 100 | 19 | 25 | +6 (near the peak) |
  | 200 | 50 | 50 | 0 |
  | 400 | 132 | 100 | -32 |
  | 600 | 233 | 150 | -83 |

  A claim of 200 tiles or fewer feeds itself forever, awake or dreaming. Beyond that, land is prestige you pay for by playing.
- **Fading.** With an empty vault, the kindled tile furthest from the hearth fades back to the commons every 2 minutes. The core never fades.
- **Land remembers its shape.** Faded tiles keep a faint outline of your hue for 30 days and re-kindle at half price.
- **Cold hearths and ruins.**
  - A hearth whose soul has been away 30 days goes cold: its light goes out and its land fades.
  - A cold hearth keeps its place and its vault for 30 more days, and relights for 10 glim.
  - After that it is a ruin: a scavenge node returning 30% of its cost and of its vault. The rest goes to the Dark, and the spot is free.
- **Offline.** In v0.2 everything is indestructible. From v0.4, while the owner Dreams, only the outer kindled tiles can be contested, at 25% effect.

### The economy

Faucets and sinks are designed in pairs. The table gives starting targets per active player-hour. They are checked against `/stats`, which reports glim minted and burned per hour, and tuned first through moss yield.

| Faucets | glim/h | Sinks | glim/h |
|---|---|---|---|
| Glow-moss and crystal | 200 | Kindling: stakes, enclosures, re-kindles | 100 |
| Hushlings and dawn motes | 50 | The Dark's half of dropped glim | 40 |
| Wellspring share | 60 | Throws that hit | 30 |
| Land yield (100 tiles) | 25 | Rekindle | 30 |
| | | Upkeep (100 tiles), lanterns, hearth | 30 |
| **Total** | **335** | **Total** | **230** |

The target net is about **+100 glim per active player-hour**. Wood and stone have their own sinks: building, removal at half refund, and drops.

## 9. Progression

There are two layers. The **world layer** (resources, gear, claims, Vows) is what an Age would renew. The **soul layer** (skills, records, Depth, the Codex, cosmetics, titles) is forever. What persists is meaning, not power.

**Proposed (open question 6): skills unlock options and looks, never stats, and gear stays within 0.8-1.3x of the bare hand.** Why: skill over grind (Q4). A level-99 veteran in the best Form hits at most 1.3x as hard as a newcomer.

### Skills (all-time, 1-99)

| Skill | XP from | Unlocks |
|---|---|---|
| Hewing | Wood gathered (resonant counts double) | Ring-out cosmetics, oak planks for doors |
| Delving | Stone and crystal | Stone walls (15), roads (25) |
| Kindling | Tiles kindled, pieces placed | **Lantern (5, v0.2)**, hearth flames (cosmetic) |
| Tending | Crops | Glowberry (10), Lanternroot (25) |
| Valor | Damage to rivals and creatures; kills weighted by the victim's Flame and streak; Wellspring glim taken | Forms: Thornknuckle 10, Glaive 20, Sling 30, Maul 40 |
| Wayfaring | Tiles run at speed, drift exits, wall-kicks, ledge saves, Rim laps | Trails (cosmetic) from 10 |
| Voice | Chirps answered, Calls that reveal, Vigils, kin made | Emotes, voice timbres |

- **Curve.** XP to reach level L is `floor(60 × (L-1)^2.3)`, so 99 needs about 2.3M. Tune the XP per action so that 99 in one skill takes about 150 active hours and Kindling 5 comes within the first hour.
- **v0.2** tracks and saves XP for every skill, draws a level-up ring around you, and unlocks the Lantern. The other unlocks start in v0.4.
- **Dream rest.** Each offline hour adds 6 minutes of 2x skill XP, up to 2 hours stored.

### Depth (v0.2)

- **What counts.** You gain +1 Depth when you return from a **costly** death: you dropped at least 20 glim or materials, and at least 3 minutes have passed since your last counted return. Jumping off the Rim farms nothing.
- **How it shows.** As a numeral after your name, and later in every game. Halo rings appear at Depth 1, 7, 33, 108, 333 and 1000; v0.2 ships the first ring.
- **Decided: Depth is the most visible status but not a hiscore.** Why: it honours lives lived at the edge without rewarding dying.
- **The Daimon (v0.5)** forms at Depth 7: a small light beside you that answers your Call a fifth above your own tone. It has no power. It is the second cosmetic that travels between games, after hats, and it follows your wyrm snake too.

### Vows and Radiance (v0.6)

- **Radiance** is a level earned from all XP. It resets with each Age, or every 8 weeks if there are no Ages. At Radiance 5, 10, 15 and 20 a **Vow** becomes pending.
- **Choosing a Vow.** You choose it in the **Underlight** at your next Descent, from three cards drawn from how and where you died. If you do not die, you can choose by resting 10 s at your hearth instead.
- **Vows are sideways trade-offs:**
  - **Ember Step:** your dash leaves a 4-damage burning streak; dash cooldown +0.1 s.
  - **Still Water:** +40 ms on the perfect window; hits of 6 or more interrupt you.
  - **Moth Heart:** +1 tile of night sight; you are seen 2 tiles further away.
  - **Thrift:** throws cost 2 glim; throw damage -2.
- You hold at most 4. Vows are re-picked at the Sanctum and cleared when Radiance resets.

### Equipment: Forms (v0.6)

- Crafted at your hearth from materials plus ember (from Rim crystal). They wear down with use, which is a sink.
- Each changes what tap and hold mean, not how many verbs there are:
  - **Thornknuckle** (speed): a 0.25 s cycle and a 3-hit flurry.
  - **Glaive** (reach): 1.9-tile reach, and the charge becomes a 4-tile lunge.
  - **Maul** (weight): 0.8 s, armour during the swing, 2x knockback.
  - **Sling:** the charge throws a stone 12 tiles, using stone, not glim.
- **Counter triangle:** reach beats weight, weight beats speed, speed beats reach.
- Later, the **Censer**: a lantern on a chain whose damage comes from your own movement.

### Records and hiscores

Hiscores count people only.

- **Per skill and total level.**
- **Records** (tracked from v0.2, on the hub from v0.3):
  - longest drift chain;
  - fastest Rim lap;
  - most resonant hits in a row;
  - longest time at Flow 3;
  - most tiles ever held;
  - **longest-kept flame** (consecutive days with upkeep paid and no fading);
  - most glim taken from one Wellspring;
  - duel wins (v0.3).
- **Age honours** (v0.8, if open question 1 is accepted), carved into the Luciphon:
  - the **Morning Star**, who gave the most glim to the Age's Tithe;
  - Brightest Hearth;
  - Top Valor;
  - Top Wayfarer;
  - Most Witnessed.

### Codex and achievements (v0.5)

The Codex records every creature, node, place, piece and technique you have seen or done, each with a pixel portrait, forever. Finishing a page earns a cosmetic. Achievements are milestones: your first hearth, first loop, first Depth, a night survived on the Rim, a gold slingshot.

### Cosmetics (zero power, always, which is what lets them travel)

- **Slots:** Crown (head), Halo style, Cloak (palette), Trail, Voice (timbre), Hearth flame, Glow hue (land tint), Mark (badge).
- **Earned first (v0.5):**
  - The **Morning Crown**, for keeping land kindled without fading for 3 days running, or for reaching length 1,000 in wyrm. It shows on your Lumen *and* your wyrm snake, and is the first proof that hats travel.
  - Halo rings from Depth.
  - Trails from Wayfaring.
  - Flames from Kindling.
  - Codex-page sets.
- **Paid (v0.8), supporter packs:**
  - The **Lamplighter** pack: a lantern Mark, a cloak set, and your name on the Lamplighters' wall in the Sanctum forever.
  - Cosmetic bundles.
  - Never power, never XP boosts, never a gameplay currency.

## 10. Social and PvP

**Decided: a free-for-all by default, with consequences set by ring, and the friendly or aggressive choice made a real rule in v0.4.** Why: Q8 asks for solo FFA with choice, and Ultima Online lost about 70% of its newcomers without a safe centre.

### v0.1 and v0.2 rules

These are the ring table (§6), the targeting order and Glow knockdown (§7), and from v0.2 Sparks and resident restraint. Shoving in the Sanctum is harmless fun.

### Stance (v0.4)

- **Candle** (friendly, gold outline) or **Flare** (aggressive, ember outline).
- You change it only at the Sanctum shrine or your own hearth, with a 10-minute cooldown.
- New souls start as Candles. Flares gather +20%.
- A Candle's tap never picks a Lumen, except one the Candle holds Wrath against.

| Ring | Who may damage whom |
|---|---|
| Sanctum | Nobody; strikes only shove. Shoving the same person 3 times in 10 s when they have not shoved back brings a **Keeper**. |
| Glow | Flare against Flare (knockdown only) |
| Dim | Flares may strike anyone. Candles may strike Flares. |
| Rim (and Umbra) | Anyone against anyone |

- **Wrath.** A Candle struck by a Flare may strike that Flare anywhere outside the Sanctum for 30 s.
- **Branded.** A Flare who strikes a Candle first in the Dim is Branded for 5 minutes: visible at 2x sight, drops everything carried on death, and fair game for every Candle.
- **Keepers** (the guardian angels, EVE's CONCORD). Crime is possible but punishment is certain.
  - Any violation in the Glow or the Dim is answered within 2 s by a beam of light.
  - The offender is cast out to the nearest Sanctum edge with 1 Flame and marked **Outcast** for 2 minutes.
  - Anyone may strike an Outcast outside the Sanctum, and they drop as if on the Rim.
- **Brand as a consumable (v0.6).** Carry a Brand to be huntable everywhere, for +50% yield.

### Raids (v0.4)

- Structures become damageable in the Dim and on the Rim, only while their owner is awake.
- Each piece broken costs the attacker 50% of its wood or stone cost and refunds a third of it.
- A breached claim gets a 6-hour aura, and starting an attack drops your own aura.
- In Kindle mode, rivals can douse enemy kindled tiles by standing on them for 1.5 s and paying 2 glim each.

### Kin, made by offering light (v0.4)

- **Making kin.** Choose Offer light on the wheel while facing someone within 3 tiles. It costs 1 glim and puts a candle at their feet for 10 s. If they offer back within 10 s, you are kin.
- **What kin get.**
  - No damage between kin; a strike becomes a high-five spark.
  - Kin pass each other's doors, kin thorns do not hurt you, and kin land gives you the speed bonus.
  - Kin see each other at the screen edge and can Recall to each other via the Luciphon (5-minute cooldown).
  - Duet emotes, and Calls together sound as chords.
- **Limits.** At most 4 kin. Each kin beyond 3 within 12 tiles cuts everyone's yield by 10%, so groups exist but solo play stays viable.
- **Residents can become kin** (30% acceptance, unless wronged). The Codex then quietly marks them as residents.
- **Friends** are the platform list (v0.5): soul-to-soul requests to people who are not present, a list, and in-page notifications. Kin is the bond made in person, inside Luciphon.

### Vigils (v0.4)

A Candle standing within 6 tiles of a Spark while Hushlings or hostile Lumens are near earns Voice XP. The Spark sees a soft halo over whoever watched over them. That is how players become guardian angels.

### Voice and emotes (no text)

- **v0.1:** the chirp (tap the Heart: a ring of light and a pictogram, at most 2 a second).
- **v0.2:** Wave, Bow, Cheer and Sit on the wheel.
- **v0.3:**
  - eight emotes: wave, bow, cheer, laugh, sit, point here, heart, shrug;
  - the **Call**: hold the Heart 0.6-1.2 s for 2-6 glim. It lights 4-10 tiles for 2 s, reveals Hushlings and hidden Lumens to you and your kin, and gives kin in range +10% speed for 4 s;
  - the **Challenge** tone;
  - **Quiet**, which mutes the nearest Lumen's chirps and emotes for you, stored on your soul.
- **Rate limits.** One emote per 1.5 s, with a burst of 2.
- **No gloating.** Within 3 tiles of a death in the last 10 s, only Bow can be used.
- **Ground notes (v0.6).** A template plus a word from a 40-word lexicon ("beware ___ ahead", "try ___", "rest here"). They cost 1 glim and last 3 days, extended when others Resonate with them.
- **Death echoes (v0.6)** replay the last 4 s of a death on the spot, in faint light, for 1 hour.

### Duels (v0.3)

1. Send the Challenge tone to a Lumen within 4 tiles. If they answer with Challenge within 10 s, a **6-tile ring of light** forms, even in the Sanctum.
2. Inside the ring, only the two of them can hit each other. There are no drops and no Descent. First to 2 knockdowns wins. Outsiders are pushed out.
3. Spectators glow. Duel wins are a record.

These are the purest skill fights and the first thing worth streaming.

### Spectating: the Witnesses

- **v0.1.** A watcher (`?watch=1`) sees the **Sanctum view**: a fixed camera at (0,0), radius 12, live.
- **Following (v0.3).**
  - `?watch=1&follow=<public lumen id>` shows only what the followed Lumen sees, 10 s behind (Counter-Strike's GOTV). Duels and the Sanctum stay live.
  - Watcher frames are built once per followed Lumen and queued (about 300 frames, about 90 KB).
  - The room keeps a world keyframe every 2 s in a 12 s ring. A watcher who joins late starts from the keyframe 10 s back, and its chunks come from the same keyframes.
- **Stars and Bless (v0.5).**
  - A watcher with a soul is a **Witness**: a faint star in the night sky above whoever they follow.
  - Lumens see *how many* stars are above them, never who.
  - A Witness may Bless once every 10 minutes: a 5-glim mote falls visibly from the sky beside the followed Lumen.
  - At most 3 Blesses per Lumen per 10 minutes, and never within 10 s of the Lumen being hit.

## 11. Theme and meaning

**Luci** is light and **phon** is voice: the Luciphon is a bell that shines, a lantern that sings. Lucifer here is Phosphoros, the Morning Star, the one who carries light out to the edge and brings the dawn back. Every idea below is a mechanic; nothing is explained in text.

| Idea | Mechanic | When |
|---|---|---|
| **The light-bearer** | Your Flame is your life, and the dimmer you are the further you fly. Carried glim is wallet, night sight, ammunition and exposure at once. Hoarding makes you a Beacon, and giving light makes kin. The Morning Star is whoever gave the most to the Tithe. | v0.1, v0.4, v0.8 |
| **Voice** | The chirp; resonant strikes as notes; the Call that reveals and rallies; kin chords. The enemy is the Hush (darkness *and* silence), and the Unsung takes your voice away. | v0.1, v0.3, v0.6 |
| **Guardian angels** | Keepers: crime is possible, punishment certain. Vigils: players become guardians. The Daimon: the Holy Guardian Angel as a light built from your returns, answering you a fifth above. | v0.4, v0.5 |
| **Rebirth, not rainbows** | The Descent (Dissolution, the Underlight's life thread, your killer marked, the Return); Depth as status; Vows chosen in the Underlight from how you died; Rekindled grace that grows with recent loss. | v0.1, v0.2, v0.6 |
| **Watchers in other layers** | Witnesses as stars over the watched, Bless motes falling from the sky, the hub card as the sky looking down. Later, a stream's audience is a Witness sky. | v0.1, v0.5 |
| **Ancestors** | Death echoes, ground notes, the Book of Ages on the Sanctum stones, Strata (named ruins of past Ages under the new map), Ancestor statues of your last Age's Lumen. | v0.6-v0.8 |
| **Games within games** | The Ouroboros Pool, where wyrm dreams in the Sanctum; the Arcade; cabinets and consoles; the desk with monitors running other games; a monitor showing Luciphon showing the monitor. | v0.7-v1.0 |
| **The world breathes** | A deploy is the **Stillness** (the world freezes and exhales, and you return where you stood). Leaving is the **Dream**. Dawn renews the nodes. If open question 1 is accepted, an Age's end burns down what you built and keeps what it meant. | v0.1, v0.8 |

**The design rule for every layer: one lives, one witnesses, and the witness can send a little light down.** The hub watches Luciphon. Luciphon's Pool watches wyrm. The desk watches everything. The Witnesses watching you are themselves visible as stars. Each new layer must give those three roles a mechanic.

**Tone rules.**

- No sermons and no text walls.
- A dark world with one hue per soul, white for dissolution, negative for the Underlight.
- Never rainbows or colour cycling, and no literal drugs: the "mushroom trip" is the *shape* of the journey (out to the Rim, loss, return, integration).
- No stance is called "Fallen".

## 12. Persistence and live updates

### What is saved, when, where

| Data | Where | When | Worst loss |
|---|---|---|---|
| The world (from v0.2, schema 1): tiles, nodes, pieces, hearths and vaults, outlines, every Lumen (position, Flame, carried goods, XP, records, Depth, play time, last seen), residents' brains, drops, records, clock | `$DATA_DIR/rooms/luciphon/snap-<unix>.bin` | Every 10 s at a tick boundary; after genesis; a final one on SIGTERM | 10 s on a crash, nothing on a deploy |
| v0.1: Lumens only (schema 0, like wyrm's snakes; discarded when schema 1 ships) | The same | The same | |
| Journal (v0.3): hearths placed or gone cold, vault moves, records, kin bonds, cosmetic grants, later purchases | `.../luciphon/journal.log` | Records with a length and CRC32, fsynced in groups every 200 ms; each snapshot notes the sequence number | 200 ms |
| Platform souls (Stage 0): soul, name, created, last seen, play time | `$DATA_DIR/souls` | On change, at most every 5 s (tmp file, `sync_all`, rename) | 5 s |
| Visits | `$DATA_DIR/visits` | As today | |
| Browser | `secretspace/key`, `secretspace/name`, `secretspace/luciphon/taught` | | |

**Decided: no journal before v0.3.** Why: nothing is purchasable or granted yet, so up to 10 s of loss on a crash is acceptable.

### Snapshot mechanics

- **Writing.** On the room thread, `room.save()` serialises the world (target: 5 ms or less) and hands the bytes to a **writer thread**. The writer writes `.tmp`, calls `sync_all`, renames the file into place and syncs the directory.
- **Rotation.** Keep the newest 6, plus one per hour for 24 h, plus one per day for 14 days. Turn on Railway volume backups.
- **Container** (`engine::snap`): the magic `SSNP`, container version `u16`, room id, room schema `u16`, tick `u64`, unix time `u64`, payload length `u32`, payload, then `crc32(payload)`.
- **Luciphon's payload** is sections of `tag u16, len u32, bytes, crc32`: META, TILES, NODES, PIECES, HEARTHS, LUMENS, RESIDENTS, DROPS, OUTLINES, RECORDS.
  - Unknown sections are skipped and missing ones get defaults.
  - A bad CRC on any known section rejects the snapshot.
  - Each schema bump gets a `migrate_sN` function.
- **Boot.**
  - Each attempt builds a fresh room from the factory, then calls `load`: first with the newest snapshot, then with older ones.
  - `load` returns Err only for bytes that should have loaded. A room that can start over (wyrm) returns Ok.
  - **A fresh world is generated only when no snapshot exists.**
  - If snapshots exist and none loads, the room stays closed and `/health` reports it as quarantined. Its files are copied to `/data/quarantine/<unix>/` and never rotated. The room waits for a build that loads them, or for an operator to clear them.
- **Schema 1 freezes the day `persist.rs` ships (Stage 2.5)**, with its golden fixture. Production migrates from there.
- **Lumens are bounded.** A Lumen is saved only once its soul is persistent (2 minutes of play). A Lumen with under 10 minutes of play and no hearth is forgotten after 14 days.
- **Tests:**
  - `snapshot_round_trips_exactly`;
  - `hostile_snapshots_never_panic` (fuzzed, like `hostile_bytes_never_panic`);
  - `every_old_world_still_loads`, with fixtures from every schema (`crates/luciphon/tests/fixtures/world-s1.snap`, kept at 200 KB or less).

### The lifecycle of a Lumen (v0.2)

- **Leaving out of combat** (the socket closes, or 20 s pass with no Input) is an **instant Dream**.
  - The body fades in 1 s and its state is stored.
  - You wake where you began to Dream, with your bag and 3 s of ghost. If that tile has been built on in the meantime, you wake on the nearest walkable tile.
- **Lingering.** If you were hit by anything in the last 10 s, or a Lumen who may damage you is in sight when you leave, your body **Lingers**.
  - It stays for 10 s (`LINGER_TICKS = 300`), with the stick released and still vulnerable, and then Dreams.
  - Closing the tab is no escape from a fight, and there are never lootable sleepers.
- **Two tabs, one soul.** The newer connection takes the Lumen. The older one is told `Elsewhere` and becomes a watcher.
- **Guests (soul 0)** are never saved or resumed, and get one Lumen per connection.
- **Timers** (wicks, Lingers, crops) stand still during a Stillness and during the 30 s wait for souls.

### How patches reach people

| Change | Path | What the player sees | When it stops interrupting at all |
|---|---|---|---|
| **Server code** | Railway sends SIGTERM. `room.still()` freezes every awake Lumen, invulnerable, and sends `Still`. Then a final snapshot and exit. On boot, held Lumens wait 30 s for their souls and resume with 2 s of ghost. | The **Stillness**: the frame freezes and dims under a slowly gathering gold light while the page retries (after 250 ms, ×1.6 with jitter, capped at 4 s). You fade back in where you stood. Never a menu. | v0.4, the gate/sim split |
| **Page code** (look, feel, web) | CI skips `railway up` when the server's inputs are unchanged. The page fetches `dist/luciphon/version.txt` (page build and page protocol, served no-cache) after each Welcome and every 5 minutes. A newer page reloads at your next Return fade (after the respawn tap) or Dream, never during the Underlight. | Nothing: the world never restarts, and rebirth is the update. | v0.7, cartridges |
| **Laws** | The Welcome carries the server's Laws, so a page always predicts with the server's numbers. Changing them is a server push. | The Stillness | v0.3, laws as data (below) |
| **Content** | A server push | The Stillness | v0.6, content as data on the same path as laws, announced 60 s ahead as an omen in the sky |

**Laws as data (v0.3).** `laws.rs` declares every default and a `[min, max]` bound for each value. `/data/content/luciphon.laws` (`key = value`) or an HMAC-signed `POST /admin/laws` overrides values within those bounds. The change is swapped in at a tick boundary and broadcast with an effective tick and a hash.

**Q11, honestly.** Q11 is met in part from v0.1: nobody is kicked, and page-only pushes never restart the world. It is met in full only at v0.7. Until v0.4, nearly every gameplay push is a Stillness.

**Protocol.** From v0.1 the server speaks page protocol N and N-1 (one version byte and a decode branch), so a page one version behind keeps working until its natural reload. A page older than Welcome's `oldest_page_proto` shows the Stillness overlay and polls `version.txt` until it names a page the server accepts, then reloads once.

**Required on Railway:** set `RAILWAY_DEPLOYMENT_DRAINING_SECONDS=15`, turn on volume backups, and keep the volume at `/data`.

## 13. Platform hooks

1. **Soul (Stage 0).**
   - `kit::Session` creates a random 128-bit key (`crypto.getRandomValues`) and stores it in `secretspace/key`, shared by every page on the Vercel origin. The key is sent only in the first binary message, never in a URL.
   - The server computes **soul = the first 8 bytes of SHA-1(key)** and never stores the key.
   - Rooms receive a `Who { soul, name, watch, build }`. Real accounts later bind to the same soul id, with no migration.
   - **One name everywhere.**
     - The server's soul store (`$DATA_DIR/souls`) holds each soul's name, so wyrm, Luciphon and every later game share it.
     - The stored name wins. A page sends `rename` only from its name screen.
     - Names are unique after folding case and look-alikes (0/o, 1/l/i, 5/s), and residents' names are reserved.
     - The server answers Hello with a platform `Seen` (status, name) before the room speaks. A taken name sends the page back to its name field.
   - **Abuse bounds.**
     - A soul becomes persistent after 2 minutes of play.
     - Souls with under 10 minutes of play expire after 14 days away, and their name is freed.
     - At most 10 new souls an hour per address, as Railway's edge reports it in `X-Forwarded-For`.
   - **Recovery.**
     - Luciphon's first-run screen shows the key as 11 words ("write these down"), and offers a restore field (`kit::TextField`). **Built (Stage 0):** `engine::words` makes each word four letters, consonant-vowel-consonant-vowel ("bako", "rimu"), 12 bits apiece: 128 bits of key and a 4-bit check, so most slips in copying are caught. No word list to commit or fetch.
     - Device pairing (6 digits) and then passkeys follow in v0.5.
   - From v0.5 the name is chosen on the hub.
2. **Storage namespacing.** Platform keys are `secretspace/key` and `secretspace/name`; game keys are `secretspace/<game>/...`. wyrm's `secretspace/best` moves to `secretspace/wyrm/best`, with a one-time read of the old key.
3. **Cosmetic registry (v0.5).**
   - `engine::cosmetic`: `id: u16`, slot, palette, and a **12x8 fallback glyph**, so any game can at least draw any cosmetic as a badge.
   - Ownership lives on the soul, and grants go through the journal.
   - Each look crate draws what it can. The Morning Crown is drawn by `luciphon-look` on the hood and by `wyrm-look` on the snake's head, in the same release.
4. **Hub card.**
   - **v0.1:**
     - A `CARDS` entry with id `luciphon`, title `LUCIPHON`, blurb `["carry your light out,", "knock them off the edge"]`, path `/luciphon/` and hue 42. Once shown, it replaces "NEXT GAME".
     - It has a **live preview**: `hub-web` links `luciphon-look` and Luciphon's mirror and watches the Sanctum view, as `watch.rs` does for wyrm.
     - A new `hidden` flag keeps the card off the public hub until the FUN GATE passes. Until then the page is reachable only at its path.
   - **v0.3:** the preview follows the brightest awake Lumen (Flow, streak or Beacon) on the 10 s delay, and lists the top name per skill.
   - The hub keeps its honest split between people online and everyone playing (residents included).
   - **The end state of `/`.** Until cartridges, `/` stays the light card page. After them, `/` opens into the Sanctum, each card becomes a cabinet there, and the card page remains as the lite fallback.
5. **Games within games.**
   - **v0.7: the Ouroboros Pool in the Sanctum.**
     - While the Pool is on screen, `luciphon-web` opens a watcher socket to `/ws/wyrm` and draws wyrm through `wyrm-look` into the pool's ellipse.
     - Kneeling at it (a tap) plays wyrm. At first this navigates to `/wyrm/?return=luciphon`, which on death or exit returns you to your Lumen kneeling at the Pool. Later wyrm plays in place, as a cartridge.
     - Others see you kneeling, with your snake swimming in the water.
     - A wyrm score unlocks a Luciphon trail, and your halo shows on your snake.
   - **Cartridges (v0.7).**
     - Each game's page logic becomes a plain wasm32 module with an `extern "C"` interface (init, resize returning a framebuffer pointer, input, net in, net out, frame, save, load).
     - One kit host loads them through `js_sys::WebAssembly`. It is proven on the hub previews first.
     - The host can then probe `WebAssembly.validate` and load a simd128 build where the browser supports it.
   - **v0.7: the Arcade.** A store at the Sanctum sells game cartridges for glim, and a Console piece at your hearth plays any cartridge you own.
   - **v1.0: the desk and the phone.**
     - A house Nook with a desk. Sitting at it switches to a 320x180 first-person raycaster at the desk.
     - Its monitors (one, two or three, an ultrawide, a laptop, all cosmetic peripherals) are cartridge framebuffers, and they recurse.
     - A pocket **phone** runs cartridges anywhere in the world.
     - compusophyOS runs on these screens only if it can be built as a framebuffer cartridge. Check that before v1.0: a DOM-based version needs a port.
6. **Streaming and the rest (later).**
   - A soul lights a **Beacon** in any game to stream. Followers and supporters are soul-to-soul edges, separate from kin and friends.
   - Notifications ("your claim is fading", "the Wellspring wakes in 60 s", "a Beacon you follow is lit") are **in-page only**. Web Push needs a service-worker JS file (against rule 1) and outbound HTTPS with VAPID, ECDH P-256 and AES-GCM (open question 4).
   - Modular text chat comes once moderation exists. Up tag `0x40` is reserved for `Say`.
7. **Payments (v0.8).**
   - Stripe Payment Links with `client_reference_id = soul`.
   - Inbound webhooks verified with HMAC-SHA256, written in `engine` next to SHA-1, with grants through the journal.
   - No outbound TLS is needed.
8. **Instances (v0.9).** Room factories take an instance id, so `/ws/luciphon/<n>` starts on demand. The hub sums instances by id, and kin can join each other's instance.

## 14. Architecture

### Changes to shared crates (Stage 0)

**`Cargo.toml` and scripts.**

- `[profile.release] opt-level = 3`, replacing `"s"` (measured 1.6-1.9x faster). `build-web.sh` prints raw and gzipped sizes for every page. The switch stands unless a page grows more than 20% gzipped.
- A new `[profile.server]` with `inherits = "release"` and `panic = "unwind"`. Cargo forbids setting `panic` per package, hence the custom profile.
- `scripts/ship.sh` builds `--profile server` and copies `target/x86_64-unknown-linux-musl/server/server`.
- Every page ships scalar wasm. simd128 waits for the cartridge host's probe (v0.7).

**`engine`** (still zero dependencies; every module has tests):

- `room.rs`: the trait below.
- `who.rs`: `Who`, the platform `Hello` and `Seen` codecs, `clean_name` (moved from wyrm), and name folding.
- `sha1.rs` (moved from `server/ws.rs`, which then uses it) and `crc32.rs`.
- `snap.rs`: the container, and the section writer and reader.
- `fixed.rs`: `Fx` (Q16.16, saturating), the committed sin table, `atan2`, `isqrt`, `len`.

```rust
pub struct Who { pub soul: u64, pub name: String, pub watch: bool, pub build: u32 }

pub trait Room: Send {
    fn id(&self) -> &'static str;
    fn hz(&self) -> u32;
    /// A browser is here: after its platform Hello (consumed by the server), or as
    /// a guest (soul 0) on any other first message or after 1 s of silence.
    fn open(&mut self, conn: u32, who: &Who, out: &mut Outbox);
    fn message(&mut self, conn: u32, bytes: &[u8], out: &mut Outbox);
    fn close(&mut self, conn: u32);
    fn tick(&mut self, out: &mut Outbox);
    fn people(&self) -> usize;
    fn playing(&self) -> usize { self.people() }
    /// The world as bytes for a snapshot; None if nothing is worth keeping.
    fn save(&self) -> Option<Vec<u8>> { None }
    /// Called once, on a fresh room. Err only for bytes that should have loaded;
    /// a room that can start over returns Ok.
    fn load(&mut self, bytes: &[u8]) -> Result<(), &'static str> { let _ = bytes; Ok(()) }
    /// The server is about to stop: the Stillness.
    fn still(&mut self, out: &mut Outbox) { let _ = out; }
    /// Messages a browser may fall behind by before it is let go.
    fn backlog(&self) -> usize { 60 }
    /// Extra numbers for /stats.
    fn stats(&self) -> Vec<(&'static str, i64)> { Vec::new() }
    // Built in Stage 0 beside the above: who(conn, &Who) when a connection
    // says Hello again (a rename); schema() for the snapshot container;
    // reserved() for its bots' names, which no soul may take.
}
```

**The platform `Hello`** is the first message on any game socket: `0xFE`, kind `1`, platform protocol `u8`, `key [16]`, `name str`, `rename u8`, `build u32`. A page may say it again later (a rename); the server answers each one, and rooms never see a message that starts with `0xFE`.

- A page that sends anything else first opens as a guest, and its message is delivered to the room. This keeps cached wyrm pages working.
- The server answers a Hello with `Seen`: `0xFE`, kind `1`, status `u8` (ok, new or taken), `name str`. `Still` is `0xFE`, kind `2`: hold the picture and reconnect.
- Watchers (`?watch=1`) say no Hello and are opened at once, as guests.

**`server`** (std only; `main.rs` is split to stay under the cap):

- `host.rs`:
  - one thread per room, built from a factory list `const ROOMS: &[fn(u64) -> Box<dyn Room>]`;
  - a backlog per room;
  - snapshots every `10 × hz` ticks;
  - `catch_unwind(AssertUnwindSafe(..))` around every room call. On a panic it logs the panic, rebuilds the room from its factory, loads the newest snapshot, sends every connection `Still` and closes it so pages resume;
  - after 3 panics in 10 minutes it loads the snapshot before the newest. One more panic stops that room (`/health` names it), while the other rooms keep running.
- `store.rs`: the writer thread, rotation, boot load and quarantine (§12).
- `souls.rs`: the platform soul store (§13).
- `signal.rs`:
  - `extern "C" { fn signal(sig: i32, h: extern "C" fn(i32)) -> usize; }` for SIGTERM and SIGINT, setting an `AtomicBool`. Installing the handler also fixes the PID-1 problem, where the kernel ignores SIGTERM because no handler is installed.
  - A watcher thread polls the flag every 50 ms. On shutdown it calls `still` in every room, flushes for 300 ms, takes a synchronous final save, and exits 0 (forced after 5 s).
  - The blocking accept loop is simply abandoned at exit.
- `/health` returns `ok <build>`, where the build is `option_env!("SECRETSPACE_BUILD")`, set by `ship.sh` to the server-input hash. It also names any quarantined or stopped room. `/stats` adds each room's `stats()`.

**`kit`.**

- `Session`: `key()`, `name()`, `words()`, `restore(words)`.
- `Link` (`link.rs`): a reconnecting socket that sends `Hello` first on every open, retries after 250 ms (×1.6 with jitter, capped at 4 s), and reports Up or Holding.
- `pointer.rs`: Pointer Events with timestamps, one pointer at a time, and the latched stick for mouse and pen.
- `version.rs`: polls `version.txt`.
- `Screen::fit_view(min_short)`, `vibrate(ms)`, `download(name, bytes)`.

**`pixels`.**

- `Atlas` and `blit_atlas`: an integer, alpha-keyed blit with no float math per pixel.
- `blit_add`: an integer additive blit, for the baked glows.
- `light::apply`: an integer multiply with a Bayer dither.
- `invert`: the Underlight pass.

**`scripts/caps.sh`.** A line checking that `crates/luciphon` depends only on `engine`, like wyrm's.

**`CLAUDE.md`** is at 6,119 of its 8,000 characters. The Luciphon map lines, the rule-7 command (adding `-p secretspace-luciphon-web`) and the rule-3 sentence must fit, so trim other lines if needed.

### New crates (the wyrm template)

**`crates/luciphon`.** The core: std plus `engine` only, shared by server and page. It is 22 files plus `lib.rs`, each under 1,000 lines.

| File | Holds |
|---|---|
| `laws.rs` | `pub struct Laws` and `pub const LAWS: Laws`, with every world number in this spec. Code takes `&Laws`, and the Welcome carries it encoded. |
| `island.rs` | Seeded generation as in §6 |
| `tiles.rs` | Tile = `ground u8` (kind:5, level:3), `obj u8`, `claim u16` (high bit = open wick); 32x32 chunks; RLE codec; walkability, friction and void queries |
| `motion.rs` | `Body` and `step`: every movement rule, in §7's order |
| `combat.rs` | `pick`, strikes, lance, heavy, throws, knockback and stun, hazards, ring rules, knockdown, gutter, ghost, Rekindled, Sparks |
| `gather.rs` | Nodes, rings, resonance, ring-out, dwell, regrowth, pickups, bag, crops |
| `build.rs` | Pieces, placement and the walling-in checks, build mode, removal |
| `claim.rs` | Hearths, vaults and the lamp, wick, loop fill, cap, snuff, upkeep, fading, outlines, cold hearths, ruins, lodgings |
| `clock.rs` | Day and night, sight radius, dawn renewal, the Wellspring schedule |
| `creatures.rs` | Hushlings |
| `bots/brain.rs`, `bots/plan.rs`, `bots/fight.rs`, `bots/crowd.rs`, `bots/genesis.rs` | Residents: perception and goals; build and kindle plans; fighting; CROWD and Dream; genesis |
| `world.rs` | `World` and the `World::step` order: one Intent per Lumen, then motion, actions (with lag compensation), hazards and knockback, deaths and respawns, nodes and claims (each second), clock and creatures, crowd (each second), history ring |
| `predict.rs` | Client prediction and reconciliation |
| `proto.rs` | `Up` and `Down` codecs |
| `view.rs` | Per-viewer interest and frame building |
| `mirror.rs` | The page's rebuild of tiles and entities |
| `persist.rs` | Snapshot sections and migrations |
| `room.rs` | `impl Room`: id `"luciphon"`, `hz` 30, `backlog` 120 |

`Intent { heading: u16, throttle: u8, verb: Verb, aim: u16, aim_len: u8 }` is the *only* way any Lumen acts, person or resident. `bots_use_only_what_a_thumb_can` checks that resident intents pass the same validator and rate as human ones.

**`crates/luciphon-look`** (depends on `luciphon` and `pixels`; linked by the hub from v0.1):

| File | Draws |
|---|---|
| `palette.rs` | The palettes |
| `atlas/lumen.rs`, `atlas/ground.rs`, `atlas/things.rs`, `atlas/fx.rs` | Sprite sheets as text grids, one per file. The indigo silhouette variants are derived at decode. |
| `ground.rs` | The per-chunk ground cache, claim tints, borders, outlines |
| `things.rs` | Objects and entities in row order, with front faces and the 35% occlusion fade |
| `light.rs` | Day light sprites and the night light map |
| `fx.rs` | Hit-stop, shake, sparks, streaks, skid marks, slingshot sparks, rings |
| `hud.rs` | In-world arcs, motes, edge markers, the Heart, the wheel, bag icons, the Underlight and its thread |
| `examples/frame.rs` | A native render benchmark from a fixture world |

**`crates/luciphon-web`** (depends on `luciphon`, `luciphon-look` and `kit`):

| File | Does |
|---|---|
| `lib.rs` | Boot, the frame loop capped at 60 fps, Link, and `?perf=1` (frame timings drawn in a corner and written to `document.title`) |
| `laws.rs` | `pub struct Feel`: gesture thresholds, camera, effects |
| `controls.rs` | Two hands to Inputs (revised: first person), pure and tested natively |
| `input.rs` | Gesture to `Input` and `Heart` messages, haptics, and `?trace=1` (records the pointer stream for download) |
| `state.rs` | Mirror, prediction, interpolation, clock lead |
| `first.rs` | Name screen, recovery words and restore, ghost-thumb hints |

**Page and deploy.**

- `web/luciphon/index.html`: a copy of wyrm's, with `import init from "./pkg/luciphon.js"; init();`.
- `build-web.sh`: `page luciphon_web luciphon luciphon web/luciphon/index.html`, and a `version.txt` for each game page.
- `web/vercel.json`: the pkg path, and a no-cache rule for `version.txt`.
- Check that `deploy.yml`'s RELAY injection covers the new page.

### The wire (protocol 4)

*Protocol 4 (gear) adds gear worn and carried to your own state (3 + 12 bytes), the wand's haste to your action, the Heart's CRAFT (6), EQUIP (7) and DROP (8), and the Got event (kind 25: who, made or found, which piece).*

*Protocol 3 (the wand) adds the Input's sprint bit (bit 5 of the verb byte), the body's winded and sprinting flags, and the Beam event (kind 4: shooter, aim, length in eighths of a tile, and whether it pierced).*


*Protocol 2 (first person) adds the Input's jump bit (bit 4 of the verb byte), a Lumen's height (`Ent.z`, field bit 64) and the body's height and climb in your own state, and drops the drift fields. A Join older than the server's oldest gets a Welcome with no Lumen; the page then loads the newest build.*

**Up**, browser to server:

| Tag | Message | Bytes |
|---|---|---|
| `0xFE` | Platform Hello (consumed by the server) | See above |
| 1 | `Join { proto u16 }`: play, resuming my Lumen or waking a new one | 3 |
| 2 | `Input { seq u16, heading u16, mag u8, verb u8, aim u16 }`, at 30/s | 9 |
| 3 | `Heart { act u8, arg u8 }`: chirp, emote, wheel slot, kindle on/off, rekindle, recall, build piece; (v0.3) call ms/20, challenge | 3 |
| 4 | `Device { w u16, h u16, touch u8 }`: used for `/stats` fairness only | 6 |
| 5 | `Ping { t u32, rtt u8 }`, once a second. `rtt` is the page's measured round trip in ms/4. | 6 |
| `0x40` | Reserved: `Say` | |

How the `Input` fields are read:

- `verb`, low 3 bits: 0 none, 1 tap, 2 flick, 3 hold start, 4 release, 5 cancel. Bit 3: the stick is down.
- `mag` means different things by verb:
  - normally, the throttle: 1-254 is a walk at mag/254 of walk speed, and 255 is a run;
  - on hold start, the ticks since the press (`held_for`);
  - on release, 0 for a heavy, or the throw's range (1-255 maps to 4-9 tiles).
- `aim`: the flick or aim heading.

**Down**, server to browser:

| Tag | Message |
|---|---|
| `0xFE` | Platform `Seen { status u8, name str }`, sent before the room speaks |
| 1 | `Welcome { proto u16, oldest_page_proto u16, build u32, laws_hash u32, laws [u16 len, bytes], tick u32, hz u8, seed u64, you u16 }` |
| 2 | `Frame` (below), 30/s |
| 3 | `Chunk { zone u8, cx u8, cy u8, rle... }`: a keyframe when a chunk becomes known, at most one per tick, nearest first |
| 4 | `ChunkGone { zone, cx, cy }` |
| 5 | `Still` |
| 6 | `Board { people u16, awake u16, records... }`, every 2 s |
| 7 | `Pong { t u32 }` |
| 8 | `Elsewhere` |

A `Frame` carries, in order:

1. `tick u32`, `ack u16`, `repeats u8` and `queue u8` (the depth; the high bit means an Input was dropped).
2. **Self** (exact, about 30 bytes; absent for watchers): `x, y, vx, vy` as `i32` Q16.16; `facing u16`; `state u8`; `state_ticks u8`; `breath u8`; `flame u8`; `glim u16`; `wood u16`; `stone u16`; `flow u8`; `flags u8` (ghost, spark, kindle, build, wick).
3. **New entities:** `id u16`, `kind u8`, `x, y i16` (1/256 tile, exact for anything within 128 tiles), `facing u16`, `state u8`, `flame u8`, `glim_bucket u8`, `hue u8`, plus `name str` for Lumens.
4. **Moved entities:** `id`, `mask u8`, then only the masked fields.
5. **Gone:** `[id]`.
6. **Tiles:** `[index u16, tile u32]`, changes in known chunks.
7. **Events:** strike, hit, whiff, resonant, chirp, emote, gutter, placed, loop closed (n), snuffed.
8. **Far:** `[id, kind, tx i8, ty i8]`, lit things beyond sight.

- The clock and the Wellspring site follow from `tick` and `seed`, so they are never sent.
- The `human` flag is **not** on the player wire; residents blend in. It stays in server data, `/stats` and the snapshot.
- The mirror reproduces every tile (as the viewer may know it) and every quantized entity exactly. `the_browser_rebuilds_every_tile_and_body_exactly` enforces it.

### Interest management

- **Chunks.** A chunk becomes known when it intersects the square of half-side `sight + 4` tiles around you. It is kept while within 2 chunks and dropped beyond that, with `ChunkGone`.
- **Entities.** Sent within your sight radius, with hysteresis: they enter at `r <= sight` and leave at `r > sight + 1`. At night an entity is also visible:
  - when it stands on a lit tile within 10 tiles;
  - for a Lumen carrying over 150 glim, within 2x its light radius plus 2 tiles.
- **Far markers.** Lit things within 24 tiles: Beacons, Lumens at Flow 3, revealed Lumens, the Wellspring, hearths, lanterns.
- **Wicks.** Open wick tiles go only to their owner and to viewers who can see them. Everyone else gets those tiles as commons until then.
- **Watchers.** The Sanctum view in v0.1; from v0.3, the followed Lumen's view, delayed (§10).

### Budgets

| Where | Budget |
|---|---|
| Server world step: 16 residents, 24 Hushlings, 100 people | 1.5 ms or less (wyrm measured 0.11 ms for 18 bots) |
| Frame per viewer | 30 µs or less; 100 viewers in 3 ms |
| Whole tick | 5 ms or less of the 33 ms |
| Snapshot | Serialise in 5 ms or less on the room thread |
| Bandwidth | 8 KB/s or less per viewer on average; every message under MAX_FRAME (64 KiB); a chunk keyframe 6 KB or less after RLE |
| Client draw on a 390x844 phone at 4x throttle, sustained over 10 minutes | 8 ms or less of Rust drawing: tiles and objects 4.0, light 1.5 (night only), entities and effects 1.5, HUD 0.5. putImageData (0.7) is extra. Prediction replay 0.2 ms or less. At most 60 fps. |
| Lights and shapes | 64 or fewer lights; 40 or fewer float antialiased shapes a frame, HUD only; world objects and glows use integer blits |
| `luciphon-web` wasm | 150 KB or less gzipped |

## 15. The first playable: v0.1 and v0.2

**v0.1, the feel** (Stages 0 and 1, about 3-4 weeks), ends at the FUN GATE. **v0.2, the hold** (Stages 2-4, about 4 weeks), starts only after the gate passes.

Every stage ends green on:

- `cargo test --workspace`;
- both clippy runs (native and wasm32, including `-p secretspace-luciphon-web`);
- `fmt`;
- `caps.sh`.

Browser checks are Playwright scripts kept in the scratchpad and **never committed** (rule 1).

### Stage 0: platform plumbing, proven on wyrm

- [x] **0.1** `Cargo.toml` profiles (opt-level 3, `[profile.server]` with unwind); `ship.sh` builds that profile; `build-web.sh` prints raw and gzipped sizes.
- [x] **0.2** `engine`: `who.rs`, `sha1.rs` (moved), `crc32.rs`, `snap.rs`, `fixed.rs`.
  - Tests: SHA-1 and CRC32 vectors; snapshot round trip and hostile bytes; `fixed` agrees with `f64` within 1/4096 over 100k samples (test only), with golden values pinned.
- [x] **0.3** The new `Room` trait; wyrm updated to take `&Who`.
- [x] **0.4** `server`:
  - split into `main`, `host`, `store`, `souls` and `signal`;
  - the factory list; Hello to `Who` and `Seen`, with the guest fallback;
  - the soul store with unique names and its abuse bounds;
  - a backlog per room;
  - snapshots: writer thread, rotation, a fresh instance per load attempt, quarantine;
  - `catch_unwind`, rebuild, and the 3-panic rule;
  - the signal watcher;
  - `/health` and `/stats`.
  - Tests: `host_survives_a_panicking_room` (a test-only room that panics on tick 5 leaves a second room ticking) and `a_room_that_keeps_panicking_is_stopped`.
- [x] **0.5** `kit`: `Session` (with words and restore), `Link`, `pointer` (with the latched stick), `version`, `Screen::fit_view`, `vibrate`, `download`, and the `secretspace/wyrm/best` migration.
- [x] **0.6** wyrm proves it:
  - `wyrm-web` uses `Link`, sends Hello, and takes its name from `Seen` (a taken name returns to the menu's name field).
  - On `Still` or a close, it keeps its last frame dimmed while it reconnects. `Join` resumes.
  - It polls `version.txt` and reloads at the menu after a death.
  - wyrm's `save` and `load` keep human snakes by soul, plus bots. After a load, a human snake waits frozen and harmless for 30 s for its soul, then bursts.
  - An unknown soul spawns normally.
- [x] **0.7** CI:
  - `SERVER_HASH` = sha256 over `git ls-files -s` (blob hashes, so it covers contents) of `crates/engine crates/server crates/wyrm crates/luciphon Cargo.toml Cargo.lock scripts/ship.sh`, plus `rustc -V`;
  - `curl` the deployed `/health` (RELAY's `wss://x/ws` becomes `https://x/health`) and skip `railway up` when the hashes match;
  - write `version.txt` into `dist`;
  - set `RAILWAY_DEPLOYMENT_DRAINING_SECONDS=15` on the service.
- [x] **0.8** The `caps.sh` line for `crates/luciphon`, with a stub crate so the line runs.
- **Done when:** with a wyrm player mid-game, a page-only push leaves the server running and the open page reloads at its next death, and a server push brings the snake back at the same length. Measure and record how long the Stillness lasts (it depends on Railway's volume handoff).
  - *Measured 2026-10-07: on a server push, a scripted wyrm player got `Still`, the live server was unreachable for about 7 s (Railway's volume handoff), and the player took back its snake from the save (its id was one the save restored) about 17 s after `Still`, through a slow proxy. Locally, a browser player's snake came back at the same place and length about 1 s after a restart, the page dimmed and "holding still" meanwhile.*

### Stage 1: the feel, then the FUN GATE

- [x] **1.1** `crates/luciphon` with `laws.rs` holding every Stage 1 number, including `sparks: false`; `luciphon-web/src/laws.rs` with `Feel`.
- [x] **1.2** `island.rs` and `tiles.rs`: the §6 island from a seed (trees and rocks are solid but not yet gatherable), and the RLE codec.
- [x] **1.3** `motion.rs`: the §7 step, covering walk and run, carving, skid, drift, slingshots, spin-out, dash and i-frames, wall-kick, teeter and save, terrain, weight, body push, and Flow.
- [x] **1.4** `combat.rs`:
  - targeting, closing-speed damage with the skid carry, KB, flight and stun, launcher, lance;
  - heavy with the perfect window and overcharge; throws with glim cost and miss pickups; charge interrupts;
  - brambles, wall slam, void;
  - ring rules, Glow knockdown with its protections, gutter and drops with the Dark's half;
  - the Descent, respawn choice, ghost, Rekindled.
- [x] **1.5** *(built with 45 synthetic traces; the owner's real ones, recorded with `?trace=1` and saved with the Heart, replace them)* `gesture.rs` and `gestures_read_as_recorded`:
  - at least 40 traces recorded with `?trace=1` on real devices, a cheap Android phone among them, and committed as text fixtures;
  - they include jittery taps, slow lifts, press-flicks, holds with aim, cancels, presses that rest and then move, latched-stick runs, and **fast drift swings through the origin that must not read as flicks**.
- [x] **1.6** `proto.rs`, `view.rs`, `mirror.rs`, `predict.rs`.
- [x] **1.7** `world.rs` and `room.rs`:
  - 30 Hz, Lumens keyed by soul;
  - `Join` spawns in the Sanctum with **30 glim**, so throws are testable;
  - a close removes the Lumen;
  - save and load of Lumens only (schema 0), and `still`;
  - the server's `ROOMS` gains `luciphon`.
- [x] **1.8** `bots/` v0: 8 sparring residents with no homes. They:
  - run, drift, chase, strike, heavy and throw;
  - dodge with the clearance fan-out (void, brambles, incoming motes) and flee;
  - perceive 250-400 ms late, reading positions 8-12 ticks back from the history ring;
  - err by 6-12 degrees in aim.
- [x] **1.9** `luciphon-look`:
  - palette, atlas, the ground cache, objects with front faces in row order, the Lumen sprite set;
  - day light and the Luciphon's light;
  - every §3 feel effect, as baked sprites;
  - Flame and breath arcs, motes, the Luciphon edge marker, the Heart;
  - the Underlight (invert, thread, killer).
- [x] **1.10** `luciphon-web`:
  - `fit_view(352)`, `Link`, Hello and Join;
  - gesture to Input at 30 Hz, the latched stick, prediction, 66 ms interpolation, haptics;
  - the Stillness overlay;
  - the first-run screen (name via `kit::TextField`, recovery words, restore) and ghost-thumb hints;
  - `?perf=1` and the 60 fps cap.
- [x] **1.11** `web/luciphon/index.html`, the `build-web.sh` line, `vercel.json`, the RELAY injection, and the hidden `CARDS` entry with its live Sanctum preview.
- [x] **1.12** Tests:
  - `hostile_bytes_never_panic` (Up and Hello);
  - `the_browser_rebuilds_every_tile_and_body_exactly`;
  - `prediction_matches_the_server`: 10k ticks of random inputs across every terrain, with drifts, dashes, wall-kicks, teeters, repeats and dropped inputs, compared bit for bit;
  - `bots_use_only_what_a_thumb_can`.
- [x] **1.13** **Legibility and budget gates.** *Measured 2026-10-07 (Playwright, 390x844, DPR 3, 4x CPU throttle): a 12x16 Lumen reads, hood hue and core clear, so 16-px tiles stand (hats do not exist yet; that half waits for cosmetics). A whole frame drew in 18 ms (Stage 4.1 must bring the Rust drawing under 8). A full-screen light multiply at quarter resolution cost 14.5 ms, far over 1.5: **night takes the per-tile fallback** (8 pre-shaded levels).*
  - Screenshot a Lumen wearing a test hat at 390x844, DPR 3, and judge it at arm's length. If it fails, switch to 20-px tiles with a 9-tile day sight, or a 16x24 Lumen. Record the decision with the screenshot.
  - Measure the night light pass at 4x with `?perf=1`. If it is over 1.5 ms, take the per-tile fallback.
- **Proxy check (an agent can run it).** A 10-minute headless run of 8 residents logs technique counts. It passes when every §4 technique except the feint occurs, and deaths have at least three different causes.
- **FUN GATE (the owner).** Three 10-minute sessions against residents, plus anyone else available: a phone one-handed, a trackpad, a mouse. Each session ends with a one-line report: kept playing or not, the best moment, the worst moment.
  - **Pass:** the owner wanted to keep going on all three devices, and names a moment on each (a slingshot escape, a launcher into brambles, a ledge save). The card is then un-hidden.
  - **Fail:** tune the motion and combat laws and run it again.
  - **v0.2 does not start until it passes.**

### Stage 2: the hold

- [x] **2.1** `gather.rs`: the §8 nodes, ring multipliers, resonance and ring-out, dwell, regrowth, pickups, bag and weight, glim cap and motes, Beacons; the Planter and Sunwheat.
- [x] **2.2** `build.rs`: hearth placement and core, the walling-in checks, vault, lamp and banking, build mode, wall, door, thorns, lantern, planter, removal; lodgings.
- [x] **2.3** `claim.rs`:
  - kindle mode, wick and stake, the 48-tile limit, the 60 s timeout;
  - close and flood fill (the 64x64 box, the 300-tile and cap limits), snuff;
  - speed bonus, land yield, upkeep, fading, outlines and half-price re-kindle, cold hearths, ruins.
- [x] **2.4** The wheel (§4) with Build's second ring, Kindle, Rekindle, Recall and four emotes.
- [x] **2.5** `persist.rs`:
  - sections META, TILES, NODES, PIECES, HEARTHS, LUMENS, DROPS, OUTLINES; `save` and `load`;
  - **freeze schema 1** and commit `world-s1.snap`;
  - tests: `every_old_world_still_loads`, `snapshot_round_trips_exactly`, `hostile_snapshots_never_panic`.
- [x] **2.6** The lifecycle: Dream, Linger, the 20 s idle Dream, waking where you dreamed, two tabs, guests; hearth or Luciphon respawn chosen in the Underlight.
- [x] **2.7** Sparks switched on; new souls start with 0 glim; skill XP saved for every skill, the level-up ring, and the Lantern at Kindling 5.
- **Done when** a phone player:
  - builds a hearth from nothing in 5 minutes or less;
  - closes a 20-tile loop;
  - plants and harvests Sunwheat;
  - dies in the Dim and sees half the bag dropped;
  - after a server push mid-loop, comes back with bag, hearth and land intact.
  - *Built 2026-10-07 and proven by `crates/luciphon/tests/hold.rs`, which plays the whole of it through the world from a soul with nothing: wood and stone struck from a birch and a rock, a hearth through build mode, a kindled loop of 20+ tiles, a planter's Sunwheat ripened and struck, a fall in the Dim leaving half the bag (half its glim to the Dark), and a save and reload that keeps bag, hearth and land. On a phone the wheel, Build's ring, the ghost and kindling were checked in a browser; the 5-minute timing on a real phone is the owner's to try.*

### First person (the owner's revision after Stage 2)

- [x] **F.1** Movement rewritten for first person (`motion.rs`): direct acceleration and stopping, jump and gravity, falling off the edge into the Dark, the dash the way you move; facing is where you look. Drift, slingshots, spin-outs, teeter and the ledge save removed. Laws in §7.
- [x] **F.2** The wire, protocol 2: the Input's jump bit (verb byte bit 4), height on the Lumen (`Ent.z`, field 64) and in your own body; the grammar relaxed to strikes and dashes while moving (`thumb.rs`); residents aim where they look and hop while chasing; the server's sight raised to 24.
- [x] **F.3** `kit::gl` (WebGL2 from Rust, a pixel layer over it) and `kit::input` (keys, every finger, the mouse's buttons and movement, pointer lock); `pixels` layers keep premultiplied alpha.
- [x] **F.4** The page in 3D (`luciphon-web`): `controls.rs` (desktop and twin thumbs), `scene/` (chunk meshes, objects, Lumens, lights, fog, sky, sparks, the hand, the Underlight in negative), `hud.rs`, the title over the island turning.
- **Done when** on a desktop and a phone you can join, look, run, jump, strike a birch for wood, dash, charge a heavy, throw, see the residents and fall off the edge into the Underlight.
  - *Built 2026-10-07. Played in headless Chromium on a desktop (pointer lock, WASD, jump, strike, a walk of 53 tiles off the Rim into the Underlight and back at the Luciphon) and on a 390x844 phone (twin thumbs through CDP touch: the stick, a look, a tap that struck a birch for wood, the jump button). Charges, heavies and throws are proven natively (`controls.rs`, `combat.rs`); `running_off_the_rim_falls_into_the_dark` and `prediction_matches_the_server` (bit for bit, with jumps) hold. A frame takes 1-3 ms of CPU. How it feels on a real phone is the owner's to try.*

### The MMO turn (owner, after first person)

The owner asked for more than an arena: something like WoW and RuneScape. Decided with the owner: **monsters and loot first**, **gear both crafted and dropped**, and **PvP only in the Dim and the Rim**.

- [x] **M.1** Sprint on shift (breath), the dash on Q; on a phone the stick pushed to its edge sprints.
- [x] **M.2** The wand: bolts, great beams and lances of light instead of melee (§4).
- [x] **M.3** Gear (`gear.rs`, the table in `laws::GEAR`): a wand, a robe and a charm worn, twelve pieces carried, each adding damage, reach, haste, armor, Flame or breath. Seven pieces crafted from wood, stone and glim at a skill level, within 6 tiles of the Luciphon or your hearth; the rare and radiant ones only drop. Saved in two new sections (GEAR, LOOT) that older saves simply lack. The page's gear panel (Tab or I; the phone's bag button): worn, carried (wear or drop), materials, recipes, skills.
- [ ] **M.4** Monsters in the Dim and the Rim, fighting back, dropping glim, materials and gear.
- [ ] **M.5** PvP only in the Dim and the Rim.

### Stage 3: alive

- [ ] **3.1** `clock.rs`: the 15-minute day, sight changes, the dusk toll, dawn renewal, hourly pulses.
- [ ] **3.2** `creatures.rs`: Hushlings, the lit-tile rule, and snuffing unfed lanterns.
- [ ] **3.3** The Wellspring: the dusk mark, a 90 s eruption, the shared pour.
- [ ] **3.4** `bots/` in full:
  - **Wanderers.** 16 persistent residents with names from a committed list of about 200 that look like typed names, plus hues and tiers (novice, steady, bold).
  - **Schedules.** Gather by day. Haul at 70% of the bag or at dusk. Build up to a 6-10 piece plan. Kindle small 5x5 to 9x9 loops at midday. At night go home or, if bold, go to the Wellspring.
  - **Homes.** 10 residents have hearths at radius 26-40, with claims capped at 60 tiles. The other 6 are lodgers who build when a spot frees.
  - **Grudges.** 8 entries kept for 48 h. They retaliate if they are stronger and avoid you if not.
  - **Habits.** They chirp back 70% of the time and wave back, sit by their hearth at night, overcook a drift 5% of the time, hit 40-80% of resonances, and never start a fight with a Spark.
  - **Crowd.** CROWD 16 and MIN_BOTS 4. Residents step aside by walking home and Dreaming. A Dreaming resident lives at 1 Hz in abstract time: its vault gains its gathering rate, it pays upkeep, and its plan advances one piece per in-game day.
  - The RESIDENTS section in the snapshot.
- [ ] **3.5** Genesis, whenever the room boots without a snapshot or without RESIDENTS.
- [ ] **3.6** Depth and the first halo ring; records in a RECORDS section.
- [ ] **3.7** `/stats` gets a `luciphon` block:
  - people and awake residents;
  - glim minted and burned per hour by source, total glim (carried, vaults, drops), and wood and stone totals;
  - claims, tiles kindled, hearth occupancy;
  - deaths by cause, and strikes landed by device (touch or mouse).
- **Done when:** a fresh boot shows resident hearths, walls and loops; at night Hushlings press on unlit ground and the Wellspring draws residents into a fight; and `/stats` shows minting and burning near the §8 targets.

### Stage 4: proof and ship

- [ ] **4.1** `luciphon-look/examples/frame.rs` as a native benchmark. A scratch Playwright run at 4x CPU throttle on 390x844, at night during a Wellspring, for 10 minutes, must show 8 ms or less of Rust drawing through `?perf=1`.
- [ ] **4.2** A scratch Playwright session with two desktop contexts and one phone context (`hasTouch`, `isMobile`, `deviceScaleFactor: 3`, 390x844): join, gather, hearth, loop, fight, die and return. Then a server push mid-session, after which all three resume with hearth, land, bag and position intact.
- [ ] **4.3** `CLAUDE.md`: map lines for `crates/luciphon`, `-look`, `-web` and `web/luciphon`, and the rule-7 command, within 8,000 characters.
- **Done when the first-minute test passes on a phone, one-handed:** a shove in the Sanctum within 10 s, a resonant strike within 30 s, a slingshot within 60 s, and by the end of the first night the owner asks for one more.

**Not in v0.1 or v0.2:**

- sound, the Call, duels;
- stances, Keepers, raids, dousing, kin, Vigils;
- skill unlocks other than the Lantern; Glowberry and Lanternroot; heights; Forms, Vows, Radiance; the Codex;
- cosmetics, Witness stars, following, laws as data, the journal;
- the Umbra and region growth, camps, bosses, notes and echoes;
- the Pool, the Arcade and cartridges.

## 16. Roadmap after v0.2

1. **v0.3, Voice and memory.**
   - `kit::Sound`: bells, pentatonic, a voice per soul, notes on resonance, the toll, the Hush as silence.
   - The Call, eight emotes with Quiet, duels.
   - Following with the 10 s delay, and the hub card that follows the brightest Lumen.
   - **Laws as bounded data**, with hot swap and the rule-3 amendment.
   - Heights: 2-3 levels, cliffs, ramps, ledge drops, +25% knockback when striking downhill.
   - Streaks with bounties.
   - The journal.
   - Hiscores and records on the hub.
2. **v0.4, Stance, kin and the gate.**
   - Candle and Flare, Wrath, Branded, Keepers and Outcast.
   - Raids with the attacker's cost, breach auras, dousing.
   - Kin by offering light; Vigils and a guiding wisp for Sparks.
   - Skill unlocks; Glowberry and Lanternroot.
   - Region growth with the Umbra, at 80% occupancy.
   - **The gate/sim split** on Railway private networking: a server push becomes a 3 s Stillness with no reconnect.
3. **v0.5, Souls.**
   - The name chosen on the hub; device pairing, then passkeys.
   - The cosmetic registry with fallback glyphs; the **Morning Crown** in Luciphon and wyrm together; the Daimon.
   - Witness stars and Bless.
   - The Codex and achievements.
   - The platform friend list, with in-page notifications.
4. **v0.6, Forms and flashpoints.**
   - Radiance and Vows; Forms with crafting, ember and wear.
   - Gloamwolf dens; **the Unsung** on a public 2-hour timer.
   - Ground notes and death echoes.
   - Content as hot-swapped data, with omens.
5. **v0.7, Layers.**
   - Cartridges, proven on the hub previews first.
   - **The Ouroboros Pool** (watch wyrm, kneel to play, rewards in both directions), then wyrm played in place.
   - The Arcade and the Console.
   - simd128 builds behind the host's probe.
6. **v0.8, Ages and supporters.**
   - *If open question 1 is accepted:* the first 8-week Age, the Long Night live event, Strata ruins, the Tithe setting the next Age's brightness, the Book of Ages stones and Ancestor statues.
   - Either way: the Lamplighters' wall and supporter packs through Stripe Payment Links and HMAC webhooks.
7. **v0.9, Many worlds.** Sticky whole-world instances `/ws/luciphon/<n>`, with kin crossing between them.
8. **v1.0, The desk.**
   - Nook interiors as zones.
   - The first-person workstation with cosmetic monitor setups running cartridges, recursion included.
   - The in-world phone; compusophyOS if it builds as a cartridge.
   - Streaming Beacons, followers and supporters; modular text chat with moderation.

## 17. Risks and how each is handled

1. **It isn't fun.** v0.1 is only the feel. The owner judges it at the FUN GATE with a written protocol, alongside an agent-run proxy check, and the first-minute test closes v0.2. Every later milestone must make the 15-minute loop better on its own.
2. **Gestures are misread on cheap phones.**
   - Every press is exactly one gesture, and flicks happen only on release, with the guard about the stick origin.
   - The recogniser is a pure function tested against traces recorded on real devices, and its thresholds live in `Feel`.
   - Every misread is cheap: a charge cancels free, an early release does nothing, a dash costs only breath, and a throw only 3 glim.
3. **Latency eats the timing windows.**
   - Deterministic windows (rings, charge length, teeter, wall-kick, lance) are judged in the predicted timeline or by sequence numbers.
   - Rewind is capped at 100 ms, there is no parry, and positional outcomes are favoured.
   - Windows are widened in laws, not in code. Measure under throttled CDP and real 4G before tuning.
4. **Prediction diverges.**
   - Saturating fixed point everywhere prediction touches, and the server's Laws in the Welcome.
   - Exactly one Input per tick, with repeats and drops reported, and `prediction_matches_the_server`.
   - Crowded fights will show corrections, because bodies and incoming knockback are not predicted. That is accepted.
5. **Knockback deaths feel brutal.** No edges in the Glow, a visible 0.5 s teeter with a save, the Rim's danger drawn loudly, deaths by cause in `/stats`, and a vault that is never at risk.
6. **The world is empty.** Residents up to CROWD, genesis, a nightly Wellspring, a small island, and an honest live card.
7. **Bots feel fake, or deceive.**
   - Reaction delays, aim error, imperfect drifts, grudges, and the human Intent path only.
   - Residents are never in hiscores and never counted as online, and the Codex reveals them after kinning.
   - Whether the hub says plainly that residents live here is open question 3.
8. **Newcomers leave (Ultima Online).** The Sanctum; Glow knockdown with 60 s of protection from the same attacker; Sparks for 30 minutes; residents who never start a fight with a Spark; a targeting rule that never auto-picks a stranger at the cone's edge; Vigils from v0.4.
9. **Persistence corrupts the one world.** CRC per section, fallback through older snapshots, a fresh room for each load attempt, quarantine instead of a silent new world, golden fixtures from the day schema 1 freezes, fuzzed loaders, rotation, and Railway backups.
10. **One panic kills every game, or loops.** The unwind profile, `catch_unwind`, rebuild from the factory, and the 3-panic rule, which rolls back once and then stops only that room.
11. **Deploys interrupt play.**
    - CI skips the deploy when the server's hash is unchanged, so gesture and look tuning never touch the server.
    - Draining is 15 s, the Stillness resumes in place, and pages reload at a Return or a Dream.
    - The gate/sim split arrives in v0.4. §12 states honestly how far Q11 is met.
12. **The economy drifts.** Faucets and sinks are a table with a target (§8), `/stats` shows minting and burning by source from v0.2, and laws hot-swap from v0.3.
13. **Identity is fragile.**
    - The server stores only the hash and one name per soul, and the newer tab takes over.
    - Safari's ITP deletes localStorage after 7 days of Safari use without a visit, and players clear site data, so the recovery words are shown on the first-run screen from v0.1.
    - Fake souls are bounded by the 2-minute rule, expiry, and the per-address limit.
14. **The render budget creeps, or phones run hot.** Fixed tiles, a cached ground, integer and additive blits, at most 40 float shapes, a measured light pass with a fallback, a 60 fps cap on 120 Hz screens, and a 10-minute sustained benchmark before each release.
15. **Tiny sprites don't read.** The Stage 1 gate decides tile and Lumen size from a real phone screenshot before the atlas grows.
16. **Scope creep.**
    - v0.1 is only the feel; stage gates and the explicit not-in-v0.1-or-v0.2 list hold the line.
    - Every milestone must make play better on its own.
    - The card stays hidden until the FUN GATE, so `claude/**` deploys never publish an unfinished game.
17. **Scripts and macros.** The server owns every window, dwell gives a casual floor, outcomes are positional, RATE limits apply, and no key combos exist.
18. **Spectators scout for friends.** Watchers get the Sanctum view only in v0.1. From v0.3 they see only the followed Lumen's view, 10 s behind.
19. **Caps and rules strain.**
    - The core is planned as 22 files, with the residents split five ways and one sprite sheet per atlas file.
    - `caps.sh` covers the new core.
    - The `CLAUDE.md` budget is watched (about 1,880 characters left).
20. **The theme is received badly.** Morning Star and rebirth framing, no "Fallen", no literal mushrooms, no preaching. The name stays a working title (open question 5).

## 18. Open questions for the owner

1. **Ages.** Every 8 weeks the outer rings, the gear and the vaults' materials would renew. Glow holds of 120 tiles or less, skills, records, Depth and cosmetics never would. Accept this, or never renew and rely on upkeep, fading and ruins instead? *Needed by v0.8.*
2. **Rule 3 amendment.** "Every number that tunes a game lives in a `laws.rs` (the core's `Laws`, the page's `Feel`), which declares its default and bounds; data may override within those bounds, swapped at a tick boundary." OK? *Needed by v0.3.*
3. **Residents.** Say openly that Luciphon has residents (a line on the card, and the Codex mark after kinning), or keep them fully indistinguishable? *Needed before a public launch.*
4. **Sign-in and push notifications.** Recovery words, device pairing and passkeys need no new dependency. Email or OAuth sign-in, and Web Push notifications, need outbound TLS (rustls in the server) or a separate service. Web Push also needs a service-worker script, which rule 1 forbids. Allow one of those, or stay without them? *Needed by v0.5.*
5. **The name.** Launch publicly as "Luciphon" with the Morning Star framing, or rename before launch? *Needed before a public launch.*
6. **Power levels.** Q9 asks for power levels. This spec keeps power in a tight band (gear at 0.8-1.3x, and skills that unlock options rather than stats) so that skill beats grind and a newcomer can beat a veteran. Accept, or widen the band? *Needed by v0.4.*