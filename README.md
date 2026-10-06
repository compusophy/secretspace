# secretspace

**One arena, everybody in it.**

Open the page, type a name, play. You are a small glowing snake in a big
dark arena shared with everyone else who has the page open right now.

- **Steer** with the mouse (on a phone: drag).
- **Eat** the glowing food to grow longer.
- **Boost** (hold the mouse button or space; on a phone, hold *boost*) to
  go twice as fast. Boosting burns your length and leaves a trail behind.
- **Run into anyone's body and you burst** into food for everyone else.
  Get someone to run into yours and their whole length is yours to eat.

The leaderboard shows the longest snakes in the arena, the minimap shows
where everyone is, and bots keep it busy when few people are on.

## How it works

The server runs the one arena: it moves every snake twenty times a second,
decides who ate what and who ran into whom, and sends each browser only
what changed in its view, a few hundred bytes a frame. The browser draws
at the screen's own rate, easing every body along its path between frames.

Everything is Rust: `crates/game` (the rules, the bots and the wire
format, no dependencies) is shared by `crates/server` (std only: its own
WebSocket, its own HTTP) and `crates/client` (compiled to wasm, drawing on
a canvas).

## Run it

```sh
bash scripts/build-web.sh                                     # dist/: the page
cargo run -p secretspace-server --release -- --static dist   # http://localhost:8787
```

Open it in two windows to play against yourself. Pushes deploy on their
own: the server to Railway, the page to Vercel (see CLAUDE.md).
