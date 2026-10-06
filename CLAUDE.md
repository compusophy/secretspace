# CLAUDE.md: secretspace

Read this first: the operating map. README.md is the pitch.

## What this is

One arena everybody plays in, in the browser: steer a snake with the mouse
or a finger, eat glowing food to grow, boost to cut people off; run into
someone's body and you burst into food for everyone else. Bots keep the
arena busy when few people are in it and step out as people arrive.

The server is authoritative: it runs the one `World` at 20 ticks a second
and sends each browser only what changed in its view. The page is Rust
compiled to wasm, drawing on a canvas.

(The earlier game, a world of browser-tab islands, still sits in
`crates/space`, `crates/web` and `crates/relay`. Nothing ships it; it is
kept until its owner decides to delete it. History: commit 70dd617.)

## Rules

1. **`crates/game` has zero dependencies.** std only; the server and the
   page both build on it, so the wire format and the laws live once.
2. **Every number that tunes play lives in `game/src/laws.rs`.**
3. **The server never trusts a browser.** `proto` decoding is bounded and
   fuzzed (`hostile_bytes_never_panic`); names are cleaned; a browser that
   floods is cut off; one that cannot keep up is let go, never waited on.
4. **Frames rebuild bodies exactly.** A known snake is sent only its new
   head points; `mirror` must reproduce the server's body point for point
   (`the_browser_rebuilds_every_body_exactly`). Never send a lossy update.
5. **Caps:** a source file holds at most 1,000 lines; this file at most
   8,000 characters. At a cap: split, shrink, or delete. Never raise one.
6. **wasm32 always green:** `cargo clippy -p secretspace-client --target
   wasm32-unknown-unknown -- -D warnings`.

## Map

```
crates/game/src    laws world (move, eat, collide, burst, food) bots grid
                   proto (wire) view (one browser's frame, the board)
                   mirror (the browser's copy) rng
crates/game/tests  arena.rs
crates/server/src  main (http, static files, ws sessions, the world thread) ws
crates/client/src  lib (dom, input, websocket) state render (canvas)
web/index.html     the page: canvas, menu, boost button
scripts/           build-web.sh (dist/), ship.sh (ship/: image for Railway), caps.sh
```

Tick (`World::step`): bots decide, everyone moves (one step, two when
boosting; boosting burns mass and drops some behind), everyone eats what
is under its mouth, heads that touch another body or the edge burst into
food, food regrows and dropped food rots, bots fill or leave.

Each tick the server sends every browser a frame (its view: snakes seen
for the first time in full, known ones as new head points, food in and
out of view); twice a second a board (leaderboard, minimap, people here).
The page draws at 60 fps, easing each body along its path between frames.

## Commands

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p secretspace-client --target wasm32-unknown-unknown -- -D warnings
cargo fmt --all --check
bash scripts/caps.sh
bash scripts/build-web.sh   # needs wasm-bindgen-cli = Cargo.lock's wasm-bindgen
cargo run -p secretspace-server --release -- --static dist   # :8787, page + server
```

Deploys are automatic (`.github/workflows/deploy.yml`) on every push to
main or a `claude/` branch: tests, then the server to Railway (`railway
up` of `ship/`, secret RAILWAY_TOKEN, variable RAILWAY_SERVICE) and the
page to Vercel (secrets VERCEL_*), pointed at the server by the variable
RELAY (`wss://<railway domain>/ws`). No RELAY, no page deploy.

Browser checks: Playwright with `executablePath` at the preinstalled
Chromium; separate contexts are separate players; `hasTouch, isMobile`
for a phone.

## Gotchas

- **`pkill -f server` kills your own shell** when the command line holds
  the pattern. Track the server's PID.
- New people are ghosts for `GHOST_TICKS`: they cannot die or kill. Without
  it, test players died within eight seconds of joining.
- Bodies are rebuilt into the grid after deaths and before spawning; a
  stale grid indexes snakes that are gone.
- The page steers by the pointer's angle from the screen's centre, where
  the camera keeps your head.
