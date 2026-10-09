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
- **A third-person wand shooter** (first person until October 2026, on
  the owner's word: "a third person shooter... fits wow as well"). Short
  battle royale matches with queues
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
      arithmetic-only noise, the shore, 400 trees, 130 rocks, 8 ruined
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
- [x] (Replaced by the design pass below.) Ten spells as loot, ranks 1-5 (a duplicate ranks one up; each rank
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
- [x] Touch controls: a stick on the left half (anywhere), the right half
      turns the view; cast (held), jump, aim (a toggle), the four spells in
      an arc about cast (icons, cooldowns), take when a scroll is underfoot.
      Health, level, XP and the feed move to the top left.

### The practice range and the title (October 2026)
- [x] A title screen (play online, or the practice range) over the island.
- [x] The practice range runs in the page itself (the same core as the
      server, no server needed): nine dummies about a ruin (some stand,
      some strafe, two spar when asked), no storm, you are hurt only when
      sparring, dummies stand again, chests are set out again.
- [x] The spellbook (B): any spell at any rank in any slot, your level, no
      cooldowns, dummies that fight back. Damage numbers on every hit.
- [x] Online: the lobby lists who is waiting; Esc (or the touch menu
      button) pauses, with leave to the title.

### The design pass (October 2026)
The owner: "a huge pass on the game design ... the icon design and spell
effect design need major over passes and the spells possible need
complete revamping ... simplicity is king! and so its beauty! and balance
of combat styles and all varieties to counter all varieties".

- [x] **Eight spells, one verb each** (the ten of Stage B are gone; ranks
      1-3, each rank +25% power and 10% less cooldown):

      | Q, E (to hurt) | what | beats | beaten by |
      |---|---|---|---|
      | Fireball | a ball that bursts where it lands (30, 3.5 m) | cover, groups | a Ward; a sidestep far off |
      | Lance | an instant beam, 80 m (34) | the still, the far, the mending | cover, a Ward, a foe in your face |
      | Frost | a fan of 7 shards (6 each), chills (60% speed, 2 s) | the runner, a sniper caught close | Gust, Blink, range |
      | Lightning | strikes where you look 0.8 s later (42, 4 m) | the still, the shielded, the hidden | anyone who moves |

      | R, F (to live) | what | beats | beaten by |
      |---|---|---|---|
      | Blink | 12 m through the air; shakes off chill | Lightning, Frost, a corner | the Lance |
      | Ward | a 40 shield for 4 s; bursts when broken | a Lance, a Fireball | patience |
      | Mend | 40 health in 2 s | the long fight, the storm | a burst; Lightning on the still |
      | Gust | throws back all within 7 m, blows their bolts away | Frost, a rush, a ledge | anything from afar |
      | Tether | a rope of light, 40 m, that hauls you where it catches (the air holds it at 24 m); jump lets go with the speed kept | a cliff, a tower, a runner, the storm | Frost (it slips), the Lance |

      The Tether came later (the owner: "high movement oriented gameplay"):
      the ninth spell, the first to move you over time rather than at once.
      Its pull lives in the body (`wandfall/src/tether.rs`, part of
      `motion::step`), so the page predicts it to the bit once the cast
      comes back; arriving hops you up onto what it caught, a jump lets go
      early as a slingshot. Its rope is lime (`fx/rope.rs`): thrown out
      slack, taut as it hauls, motes running up it.
- [x] **The wand is the heartbeat, spells the moments:** 8 a bolt every
      0.43 s; everyone drops with one spell to hurt with. Bot matches deal
      about half their damage with spells, spread over all four
      (`every_spell_has_its_place`). Bots fight at the range their spells
      like (close with Frost, far with the Lance).
- [x] **One colour and one shape per spell**, the same in its icon, its
      bolt, its landing and its marks: orange fire, a gold beam, ice-blue
      shards, a violet ring closing before the bolt, pink sparks, a blue
      bubble, green motes, a pale ring of wind. The engine's new `Rim`
      material (energy: bright edge on) draws shells, shockwaves and beams.
- [x] **Icons drawn shape by shape** (`pixels::poly`, `sweep`): a tile in
      the spell's colour, its glyph (a flame, a spear of light, a
      snowflake, a bolt, a step, a shield, a cross, the wind), rank pips,
      a clock-sweep cooldown, a flash when ready. The spellbook says what
      each beats and is beaten by.
- [x] **Reading a fight:** damage numbers in the colour of what dealt them,
      hits burst in it, every wizard's wand tip flares in the spell it just
      cast, scrolls are crystals under a pillar of their colour (taller by
      rank), your ward glows at the screen's edge, chill is shown.
- [x] The HUD: health in one bar over the spell bar (no overlap on a small
      screen); what lies underfoot on a card with its icon.
- [x] **Sound**, written by numbers (`engine::synth`, played by
      `kit::audio`): a cast and a landing for every spell, the wand, hits,
      being hurt, knockouts, levels, chests, scrolls; heard from where they
      happen. M mutes.
- [x] **Wizards** in parts: a face (eyes so you see which way it faces),
      a beard on every other one, a mantle, a belt; the wand arm rises along
      the aim to cast; walking bobs. Your own sleeve and hand in first
      person.
- [x] **Reading the end:** a burst where a wizard falls; the feed shows
      the icon of what dealt the knockout; a card when you are out or win
      (your place, who took you and with what, knockouts, level).
- [x] **The hub card** (from Stage C): the room itself, live, never a
      mock-up (on the owner's word: "why not show real gameplay"). The hub
      watches it (a watcher, not counted) and plays it in 3D: the engine
      draws the match off screen (`gpu::Offscreen`, WebGPU) over a
      fighter's shoulder (`wandfall-look`'s `Spectator`, staying with one
      until it falls or the fight moves), the picture read back into the
      card. The page and the card draw the match with the same code
      (`scene`). Without WebGPU (and while it loads) the card draws the
      island from above: places, storm, wizards, bolts, beams, falls.
      Either way: "LIVE 7 of 16 left" in a fight, "next match in 6s" in
      the lobby, "Gecko won" after. Matches always run: bots fight while
      no one is there, and someone arriving to a match of bots alone gets
      a new one at once (`bots_fight_while_no_one_is_here_and_make_way_for_someone`).
- [x] **Aim and feel:** the crosshair reddens on a wizard in the Lance's
      reach; aiming with Lightning ready shows where it will strike. Hit
      wizards flash white; your view bobs as you walk and shakes when you
      are hurt. The title shows the eight spells. The storm's wall burns at
      its edge and crackles near you.
- [x] **Wizards in full animation** (`rig.rs`), on the owner's word ("this
      is just like having a pill move around"): every wizard is jointed
      (hips, knees, waist, neck, shoulders) under a robe to the knee, its
      boots showing. It strides the way it moves (forward, back, to the
      side), knees bending through each step, arms swinging against its
      legs, leaning into a run; breathes standing; tucks its legs and opens
      its arms in the air; flinches when hit; nods with its aim; raises and
      thrusts its wand to cast; falls on its back when knocked out. The
      drop is ridden on a **broomstick** (sitting, sparks trailing from the
      bristles).
      `?orbit=ID` turns the camera about a wizard (0 is you) to look.
- [x] **Jumps and landings you feel; crouching.** Coyote time (4 ticks off
      a ledge) and a buffered press (4 ticks before landing) forgive the
      jump; holding it rises higher (lighter pull going up). Landings squat
      the rig, dip the camera, throw dust and thud, harder the harder you
      fall. Crouch (C, "duck" on phones): half speed, eyes at 1.05 m, a
      body 1.25 m tall, so bolts pass over. The wire carries it (PROTO 4).
- [x] **A wizard's island** (`places.rs`, `land.rs`), on the owner's word
      ("not wizardy enough... a wizard tower at the centre"): the **Spire**,
      a tower in a wizard's hat on a paved plaza raised 7 m, lamps about
      it, a beacon of light over it seen from anywhere, four caches of cubes at its
      foot; on a ring 76 m out, a **stone circle** (runed menhirs about an
      altar, an orb and runes turning over it), a **demon rift** (a
      scorched bowl, lava cracks, obsidian, a gate burning with runes,
      embers), a **crystal grove** (glowing clusters, giant mushrooms,
      glimmer), and later a **basalt causeway**. Each shapes the ground, blocks as trees do, and keeps two
      caches. Between them: violet and teal wizard-wood, mushrooms, ruins;
      islets float over it all. Bots step around what stands ahead.
- [x] **The Spire is climbed** (on the owner's word): a stone stair winds
      twice about the tower, a gold rail on posts at its edge, up to a
      balcony 23 m over the plaza with merlons to crouch behind; the view
      runs to every place. Decks (`places::Deck`, a stair or a ring) are
      stood on from above and passed through from below: feet stand on the
      highest surface no more than a step above them (`Map::floor`), so
      you walk up, drop off, or glide down onto them; bolts strike them.
      The stair's heading is `trig::atan2`, arithmetic only, so the page
      still predicts to the bit (`walks_up_the_spire_to_its_balcony`).
- [x] **Dusk**, on the owner's word ("everything is a little too bright"):
      a low amber sun, a violet sky with its first stars, dimmer stone; the
      lamps, crystals and lava carry the light. Grass blades are hashed
      from whole-numbered cells, so they no longer change as you walk; the
      sea's octaves are turned from each other and its glint spreads with
      distance, so it no longer crawls.
      `?cam=x,y,z,yaw,pitch` holds the camera to look (with `?orbit`).
- [x] **Spell cubes and the spellbook**, on the owner's word ("pick up and
      rank up everything you run over, no button press... the spellbook B
      to choose the spells... when players die they drop all their spells
      at the highest rank and their XP combines with yours"). Spells lie as
      cubes, their icon on every face: 36 loose across the island and a
      pair at each of 28 caches (by the places, at the ruins); no chests
      (the owner: "showing the chests instead of the spell cubes").
      Running over one learns it into your spellbook, or ranks it up (to
      III), and gives XP; a spell new to you takes a free slot of its
      kind. B opens the book anywhere, and you keep moving while it is
      open (Esc or a click off it closes it): pick a slot, then any spell
      of its kind you know (it waits 2 s before it can be cast; two slots
      trade).
      The fallen drop every spell they knew at its rank, and their XP joins
      their victor's. The wire carries the book (PROTO 5, `Up::Equip`).
- [x] **Third person, and a wizard rebuilt** (on the owner's word:
      "strafing is horrible, so is running... robotic"). The camera rides a
      spring arm over your right shoulder (`wandfall-look/src/camera.rs`):
      pulled in at once by anything behind (never inside a tree's crown, a
      stone or the tower), let out gently; nearer and tighter aiming, lower
      crouched, further out on a broom. The crosshair is cast from the
      camera, and you aim from your own eyes at what it is on. The rig is
      four modules (`rig/`: gait, pose, model, math): critically damped
      springs for everything eased; feet placed through stance and swing
      by distance travelled, so a planted foot never slides; knees by
      two-bone reach; hips turning toward where it goes while the chest
      keeps the aim; a robe to the ankle in four panels hinged at the
      waist. The spell bar sits bottom right and health bottom left, the
      middle left to your wizard. Smooth by measurement (the owner saw a
      flickering run and a wizard that all but teleported): a planted foot
      never covers more ground than a leg can reach (a run spends less of
      its stride on the ground, and runs lower), the reach is softened so
      a knee never snaps, facing and aim are eased (what the crosshair is
      on can jump), and the camera slides round trunks instead of being
      yanked in by them (`running_at_any_pace_nothing_jumps_between_frames`,
      `running_through_a_wood_the_view_never_leaps`).
- [x] **Wizards sculpted, not stacked** (on the owner's word: "the low
      poly character models need so much work... giving AI slop"). The
      engine sculpts (`render::sculpt`): distance fields blended like clay
      (smooth union, carving), meshed smooth by surface nets with each
      vertex pulled onto the surface, its normal the field's slope, its
      creases darkened by occlusion read from the field; and cloth woven
      as sheets whose folds cost nothing. A wizard (`rig/parts.rs`) now
      has a face (brows, eyes with irises, a nose, cheeks, ears), a beard
      and moustache in strands with long hair behind (or, younger, brown
      hair and a goatee), hands closed in fists about a gnarled wand, boots
      with curled toes and soles; a robe whose folds deepen to a gold hem,
      bell sleeves lined and cuffed, a scalloped mantle edged in gold, a
      curved high collar, a belt with buckle and pouch, and a hat with a
      wide floppy brim, a crumpled crown, a band and buckle. About 41,000
      triangles near and 7,000 far (beyond 16 m, coarser).
- [x] **A sculpted wood** (on the owner's word: "do the trees and rocks
      next"). `flora.rs`: broadleaf trees with ridged bark, roots flaring
      into the ground and branches up into crowns of leafy masses melted
      together and ruffled, lit as one soft mass, darker beneath; firs of
      woven tiers, ragged and drooping, darker under each; boulders cut in
      worn facets, moss on top. Near and far: within 30 m about 6,000
      triangles a tree, beyond it under 1,000 (the engine lays still
      things anew as the eye moves, each its near or far mesh, `Item::far`).
- [x] **Moving like it matters** (on the owner's word: "crouching,
      sliding, sprinting, gravity down hill, momentum"). Shift sprints
      (10 m/s against 7; forward only, not aiming, casting or wading), as
      long as stamina lasts (6 s; it comes back in 4 after a breath; run
      dry, you are winded till a third is back; a bar over your health).
      Crouching at a sprint slides: a boost (once a second), then friction,
      gravity along the slope (a hill makes a slide gather speed, to
      18 m/s), a little steering; it ends when you stand or slow to a
      crouch. Running uphill is slower and downhill faster. Speed is kept:
      overspeed bleeds away on the ground and is held in the air, so a
      jump out of a slide carries it. The rig strides longer and pumps its
      arms at a sprint; sliding, it sits low, a leg out ahead, leaning
      back, dust behind; the view widens. All of it is in the shared step,
      so the page still predicts to the bit (PROTO 6: keys are 16 bits).
      A jump is a press, never a hold (the owner: "auto bunny hopping if I
      hold space... you have to time it"); a hop pressed within 0.1 s of
      landing (or just before) keeps its speed and adds 0.6 m/s, to 12.5,
      so timed hops outrun a sprint and late ones lose to the ground; and
      once in the air a second jump turns you the way you steer, for a
      sixth of your stamina (PROTO 9). Rocks, pillars, standing stones,
      the altar and the balcony's merlons can be stood on, and climbed:
      in the air, pushing toward a top within 1.3 m over your feet, you
      pull up and over, steering held till you are over it, timed to
      come down on its middle (a short pillar takes a jump, a tall one
      an air jump too; PROTO 10). Launch runes (one by each place, eight
      in the wild, gold on the map): step on one and it throws you 18 m
      up onto your broom, to glide down wherever you steer (PROTO 11).
      Wall jumps (`wandfall/src/wall.rs`): in the air against the side of
      a rock, pillar, trunk or the tower, a jump steering away from it
      kicks you 7.5 m/s off it and 8.5 up, keeping most of your speed
      along it; three before you land, the air jump kept (pushing into
      it with the air jump to spend, the press is the air jump, to
      climb; PROTO 14).
      The **basalt causeway** (`places::causeway`, `wandfall-look/src/
      basalt.rs`), a fifth place on the ring, for all of that: six-sided
      columns packed 2.2 m apart, rising in rows about a hop each to a
      crown 12 m up among organ pipes, its far side a cliff; one in eleven
      sunk 2.4 m (a pit to hop or kick out of), sea stacks about it far
      enough apart to wall-jump between. A cache waits on the crown under
      a turning rune, one at its foot; spray blows over it. Bots back off
      what they cannot walk up, and kick out of pits (PROTO 17).
- Balance as bots play it (24 matches): every spell is in the winners'
  hands (attack 16/13/10/8 for Fireball, Lance, Frost, Lightning; life
  15/12/10/9 for Mend, Blink, Gust, Ward); the wand still deals about half
  of all damage, the spells the big moments.
- Hooks for looking (`crates/wandfall-web/src/lib.rs`): `?spells=`,
  `?nocd`, `?spar`, `?look=yaw,pitch`, `?hold=ms` (every effect held at
  that age), `?fx=name&age=ms` (any one effect shown by you, held at an
  age or again and again: `fx::NAMES`).
- **The island's sound under everything** (`wandfall-web/src/ambience.rs`,
  seamless loops written by `engine::synth`, eased by `kit::audio`'s
  hums): wind, more of it high up, on a broom and going fast; the rift's
  rumble and crackle near it; the Spire's humming chord near its tower;
  the storm's roar as its wall comes near, and all round inside it;
  crickets at night and birds at dawn and through the afternoon, hushed
  high up and in the storm.
- **Music** under the title, the lobby and the result
  (`wandfall-web/src/music.rs`): eight bars in D minor written at start
  by `engine::synth` (four chords held soft, a harp-like arpeggio, bells
  over the second half), going round; none in a match or on the range.
- **Settings** (`wandfall-web/src/settings.rs`, in the pause menu, kept
  between visits): look speed (mouse and fingers), the sound's loudness,
  and the picture (auto steps down when frames run slow; low, mid or high
  hold a tier). A short window shrinks the menu's buttons to keep them.
- **Footsteps** (`wandfall-web/src/steps.rs`): every wizard's feet as
  they come down, in time with the stride it is drawn with (`Anim`'s
  footfall), on grass, sand, stone or in the shallows; louder at a
  sprint, hushed crouching, so creeping up on someone is quiet. Yours are
  soft and close; others' come from where they are, out to 40 m.
- **Lessons on the range** (`wandfall-web/src/lessons.rs`), the first
  time: fourteen steps under the top line, each ticked off as you do it
  (move, jump, a timed hop, an air jump, a climb, a wall jump, a launch rune, a
  Tether (always in F on the range), sprint,
  slide, a wand hit, a cast, a spell cube, the spellbook), keys or fingers in their words;
  Enter or a tap skips one; the range's menu starts them again.
- **Spells that look like magic** (the owner: "still pretty low poly, not
  cool spells"), `wandfall-look/src/fx/`: fire is flowing energy and
  licking flames (a fireball a roiling ball trailing flame into smoke;
  its burst a flash, billowing flame burning down to red, a shockwave,
  streaking debris, embers and a pall of smoke); a circle of runes
  (`meshes::sigil`) where spells are cast, under a ward, under mending,
  where lightning will strike, under cubes; Lance a flowing beam wound
  with motes, flaring where it strikes; frost cut crystals in mist and
  glints; lightning forked; blink a whirl of streaks round a shaft of
  light; a ward's bubble flowing plasma that shatters into crystals;
  gust wheeling streaks and dust. Rings and circles on the ground float
  over the grass. Each cast has its gesture, so a glance at the caster
  says what is coming: fire thrust with both hands, lightning called
  down from overhead, frost swept across, a ward spread wide, mending
  drawn to the chest, a gust flung out, a blink crouched into.
- **The island's day** (`wandfall-look/src/sky.rs`, `laws::HOURS`): each
  lobby turns the island an hour, dawn (a rose sun, mist lying low), a
  golden afternoon, dusk, a moonlit night (stars, a cold moon, every lamp
  and spell bright against it), dawn again, so match after match the
  light changes. The world keeps the hour and every frame names it; the
  page turns its sky over six seconds, and eases into the storm's violet
  and out. The range stays at dusk; `?hour=dawn|day|dusk|night` holds any.
  Each lobby also rolls the weather (its own dice from the seed and the
  match's count, so the world's are untouched; PROTO 16): mostly clear,
  mist about one match in five (thick, pale, low), rain about one in
  four (grey, the sun and stars hidden, the wind up, drops streaking
  about you, a hiss under everything, no crickets or birds). The sky
  turns into it with the hour; `?weather=clear|mist|rain` holds any.

### Stage C: queues and teams
- A queue screen; matches start at N people or after a wait; duos.
- A season of names.
  - [x] The hall of wizards (`wandfall/src/hall.rs`): matches won and
        wizards knocked out, by soul (people only: no bots, no guests),
        over every match; kept in the room's snapshot (schema 1) across
        deploys, the best ten sent to every page (tag 6, bounded and
        fuzzed), shown down the lobby's left. Seasons (a reset now and
        then, the last one's best remembered) are still to come.
- Netcode at scale (`docs/engine.md` §9): 40-60 people, lag-compensated hits.
  - [x] The Lance, lag-compensated (PROTO 15): each input says the tick
        its page drew everyone else at; the world keeps where everyone
        stood the last few ticks, and a Lance strikes them where its
        caster's page saw them, never more than `REWIND` (a third of a
        second) back, and never someone already out. Bolts fly in the
        world's own time (you lead them, as anyone would a thrown thing).
