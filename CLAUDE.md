# CLAUDE.md: secretspace

Read this first: the operating map. README.md is the pitch.

## What this is

A hub of tiny multiplayer games, all Rust. The front page (`/`) shows a
card per game with live player counts, and in its footer how many people
are online anywhere and how many visits there have ever been. Each game is
its own page (`/arena/`, ...) talking to its own room on one server.

The first game, **arena**, is the template the next ones are cloned from:
an authoritative world on the server, each browser sent only what changed
in its view, and a page that draws every pixel itself into one buffer.

## Rules

1. **No hand-written JavaScript or TypeScript.** Pages are a canvas and one
   line that starts the wasm (wasm-bindgen writes its own glue). Every
   pixel is drawn by `pixels` into a buffer and shown once a frame; the
   only DOM besides the canvas is `kit::TextField`, an invisible input so
   phones offer their keyboard.
2. **`engine`, `pixels` have no dependencies; a game's core (`arena`)
   depends only on `engine`.** std only, shared by server and page.
3. **Every number that tunes a game lives in its `laws.rs`.**
4. **The server never trusts a browser.** Decoding is bounded and fuzzed
   (`hostile_bytes_never_panic`); names are cleaned; a browser that floods
   is cut off; one that cannot keep up is let go, never waited on.
5. **Frames rebuild bodies exactly.** A known snake is sent only its new
   head points; `mirror` must reproduce the server's body point for point
   (`the_browser_rebuilds_every_body_exactly`). Never send a lossy update.
6. **Caps:** a source file holds at most 1,000 lines; this file at most
   8,000 characters. At a cap: split, shrink, or delete. Never raise one.
7. **wasm32 always green:** `cargo clippy -p secretspace-hub -p
   secretspace-arena-web --target wasm32-unknown-unknown -- -D warnings`.

## Map

```
crates/engine     wire (Reader/Writer) rng room (the Room trait) hub (Stats)
crates/pixels     Canvas (RGBA buffer, AA shapes, glow, blend), font (5x7),
                  wrap, fit_scale; examples/sheet.rs draws a test sheet
crates/kit        the browser end: Screen (buffer -> canvas, pixel scale,
                  ui text scale), Socket, TextField, frames, storage, room_url
crates/hub-web    the front page (cards, live counts, scroll, footer)
crates/arena      the game: laws world bots grid proto view mirror room
crates/arena-web  its page: lib (input, socket) state render menu
crates/server     hosts every Room in its own thread, /ws/hub stats, visits
web/index.html    the hub page; web/arena/index.html the arena page
scripts/          build-web.sh (dist/: hub at /, games at /<id>/),
                  ship.sh (ship/: image for Railway), caps.sh
```

Server routes: `/ws/<room>` a game (`/ws` alone is the arena, for old
pages), `/ws/hub` the live Stats once a second, `/health`, `/stats` (JSON),
and with `--static dist` the pages. A page's first connection carries
`?v=1` and counts a visit; visits persist in `$DATA_DIR/visits` (the image
sets `/data`; a volume there keeps them across deploys).

Pixels: `kit::Screen` makes one buffer pixel `scale` CSS pixels (about 960
across at most) and the canvas shows them sharp; `ui()` is the text scale
that reads the same size on any screen (2 on a phone, 1 on a desktop).

## A new game, from the template

1. Copy `crates/arena` and `crates/arena-web` to `crates/<id>`,
   `crates/<id>-web`; rename packages; its world, wire and view are yours.
2. Its `room.rs` implements `engine::room::Room` with `id() == "<id>"`;
   add it to the `rooms` list in `crates/server/src/main.rs`.
3. Its page: `web/<id>/index.html` (copy the arena's), a `page` line in
   `scripts/build-web.sh`, a card in `crates/hub-web/src/lib.rs` `CARDS`,
   and its pkg path in `web/vercel.json`.

## Commands

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p secretspace-hub -p secretspace-arena-web --target wasm32-unknown-unknown -- -D warnings
cargo fmt --all --check
bash scripts/caps.sh
bash scripts/build-web.sh   # needs wasm-bindgen-cli = Cargo.lock's wasm-bindgen
cargo run -p secretspace-server --release -- --static dist   # :8787, everything
```

Deploys are automatic (`.github/workflows/deploy.yml`) on every push to
main or a `claude/` branch: tests, then the server to Railway (`railway
up` of `ship/`, secret RAILWAY_TOKEN, variable RAILWAY_SERVICE) and the
pages to Vercel (secrets VERCEL_*), pointed at the server by the variable
RELAY (`wss://<railway domain>/ws`). No RELAY, no page deploy.

Browser checks: Playwright with `executablePath` at the preinstalled
Chromium; separate contexts are separate players; `hasTouch, isMobile,
deviceScaleFactor: 3` at 390x844 for a phone.

## Gotchas

- **`pkill -f server` kills your own shell** when the command line holds
  the pattern. Track the server's PID.
- New people are ghosts for `GHOST_TICKS`: they cannot die or kill. Without
  it, test players died within eight seconds of joining.
- Bodies are rebuilt into the grid after deaths and before spawning; a
  stale grid indexes snakes that are gone.
- The page steers by the pointer's angle from the screen's centre, where
  the camera keeps your head.
- A ring or `outside_circle` as big as the arena still only shades the
  pixels on screen, but skip it when the edge is out of view.
- Text sized for a desktop overflows a 390-px phone: wrap it (`wrap`) or
  fit it (`fit_scale`), and check the phone screenshot.
