# secretspace

**Tiny games, everyone in them.**

Open the front page and pick a game (click it, tap it, or Tab and Enter).
Each card shows how many people are in it right now; along the bottom,
how many are online anywhere and how many visits there have ever been.
The game opens over the front page and fills the screen. Everyone who
opens a game is in the same world as everyone else who has it open.

In every game, Esc (on a phone, the menu button) opens the same menu:
back to the game, whatever the game adds (Wandfall's spellbook and
settings), a box to tell us what's broken or what you love, and the way
out.

## wyrm

You are a small glowing snake in a dark arena shared with everyone else.
Its card on the front page is the game itself, live.

- **Steer** with the mouse (on a phone: drag).
- **Eat** the glowing food to grow longer.
- **Boost** (hold the mouse button or space; on a phone, hold *BOOST*) to
  go twice as fast. Boosting burns your length, faster the longer you
  are, and leaves a trail.
- **Run into anyone's body and you burst** into food for everyone else.
  Get someone to run into yours and their whole length is yours to eat.

Bots keep the arena busy when few people are on; they graze, dodge, and
the bold ones cut across smaller snakes' paths. When people come, a bot
makes room only where no one can see it go.

## Wandfall

A wand battle royale: sixteen wizards drop onto an island on broomsticks,
pick up spell cubes, and fight while a storm closes in. Last one standing
wins. Its card on the front page is a match going on right now.

- **Move** with WASD, sprint with shift, slide by crouching at a sprint,
  time your hops as you land, jump again in the air, kick off walls,
  climb onto rocks, ride a launch rune up onto your broom.
- **The wand** (left click) is the heartbeat; right click aims down it.
- **Spells** come from cubes you run over: two to hurt (Q, E) and two to
  live (R, F), from Fireball, Lance, Frost, Lightning, Blink, Ward, Mend,
  Gust and the Tether. A second cube of a spell ranks it up. B opens
  your spellbook.
- **Levels** rise to 20 within a match; the fallen drop their spells and
  their XP is yours.

Bots fill the island when few people are on, so a match is always going.
On a phone: a stick on the left, buttons on the right, held sideways.
The practice range runs in the page itself, with dummies and lessons.

## All Rust, every pixel

- The server (`crates/server`, std only: its own HTTP and WebSocket) runs
  every game's world as a room in its own thread and sends each browser
  only what changed in its view. It trusts no browser: what it reads is
  bounded, connections are capped, every socket is pinged, and a page
  that cannot keep up is let go. What players write in a game's menu
  (and what goes wrong on a page) comes to it at `/feedback`.
- The pages draw every pixel themselves: `crates/pixels` is a software
  renderer (antialiased shapes, glows, a hand-drawn 5x7 font, accented
  letters too) writing into one RGBA buffer that is shown once a frame.
  Wandfall's island is 3D, drawn by `crates/render`, our own engine on
  WebGPU (shaders as Rust strings), with that pixel layer over it. No
  hand-written JavaScript; the page's only script line starts the
  WebAssembly.
- `crates/engine` (the wire, the room every game implements, the hub's
  numbers) and each game's core are shared, line for line, by the server
  and the page.

wyrm is the template for the next games: copy it, change the world, add a
card. CLAUDE.md has the steps.

## Run it

```sh
bash scripts/build-web.sh                                     # dist/: the pages
cargo run -p secretspace-server --release -- --static dist   # http://localhost:8787
```

Pushes deploy on their own: the server to Railway, the pages to Vercel.
