# secretspace

**Tiny games, everyone in them.**

Open the front page and pick a game. Each card shows how many people are
in it right now; along the bottom, how many are online anywhere and how
many visits there have ever been. Everyone who opens a game is in the same
world as everyone else who has it open.

## Arena

You are a small glowing snake in a dark arena shared with everyone else.

- **Steer** with the mouse (on a phone: drag).
- **Eat** the glowing food to grow longer.
- **Boost** (hold the mouse button or space; on a phone, hold *BOOST*) to
  go twice as fast. Boosting burns your length and leaves a trail.
- **Run into anyone's body and you burst** into food for everyone else.
  Get someone to run into yours and their whole length is yours to eat.

Bots keep the arena busy when few people are on; they graze, dodge, and
the bold ones cut across smaller snakes' paths.

## All Rust, every pixel

- The server (`crates/server`, std only: its own HTTP and WebSocket) runs
  every game's world as a room in its own thread and sends each browser
  only what changed in its view.
- The pages draw every pixel themselves: `crates/pixels` is a software
  renderer (antialiased shapes, glows, a hand-drawn 5x7 font) writing into
  one RGBA buffer that is shown once a frame. No hand-written JavaScript;
  the page's only script line starts the WebAssembly.
- `crates/engine` (the wire, the room every game implements, the hub's
  numbers) and each game's core are shared, line for line, by the server
  and the page.

The arena is the template for the next games: copy it, change the world,
add a card. CLAUDE.md has the steps.

## Run it

```sh
bash scripts/build-web.sh                                     # dist/: the pages
cargo run -p secretspace-server --release -- --static dist   # http://localhost:8787
```

Pushes deploy on their own: the server to Railway, the pages to Vercel.
