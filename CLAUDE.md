# CLAUDE.md: secretspace

Read this first: the operating map. README.md is the pitch.

## What this is

A hub of tiny multiplayer games, all Rust. The front page (`/`) shows a
card per game with live player counts, and in its footer how many people
are online anywhere and how many visits there have ever been. Each game is
its own page (`/wyrm/`, ...) talking to its own room on one server.

The first game, **wyrm** (snakes: eat the glow, grow, make them run into
you), is the template the next ones are cloned from: an authoritative
world on the server, each browser sent only what changed in its view, and
a page that draws every pixel itself into one buffer. Its card on the hub
is the real game, live: the hub watches the room (`?watch=1`, never one of
the people counted) and draws it with the game's own look.

Game #2, **Luciphon** (`docs/luciphon.md`), is **legacy, frozen**: it stays
as it is (live, its card hidden). **Now: the engine** (`docs/engine.md`,
UE5-class rendering on WebGPU, generic for every game) and game #3, a
wand battle royale in the spirit of Plunderstorm (`docs/plunder.md`), the
engine's first consumer.

## Rules

1. **No hand-written JavaScript or TypeScript.** Pages are a canvas and one
   line that starts the wasm (wasm-bindgen writes its own glue). Every
   pixel is drawn by `pixels` into a buffer and shown once a frame, except
   3D: WebGPU (`gpu`, WGSL) or WebGL2 (`kit::gl`, GLSL), shaders as Rust
   strings, with a `pixels` HUD layer over it. The only DOM besides the
   canvas is `kit::TextField`, an invisible input so phones offer their
   keyboard.
2. **`engine`, `pixels` have no dependencies; a game's core (`wyrm`)
   depends only on `engine`.** std only, shared by server and page. How
   it looks (`wyrm-look`) is shared by its page and the hub's preview.
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
   secretspace-wyrm-web -p secretspace-luciphon-web -p
   secretspace-showcase-web -p secretspace-wandfall-web --target
   wasm32-unknown-unknown -- -D warnings`. Every WGSL string has a naga
   test.

## Map

```
crates/engine     wire rng room (the Room trait) hub (Stats) who (Hello, Seen,
                  Still, names) words snap (save files) fixed (Q16.16) sha1
                  synth (sounds from numbers)
crates/pixels     Canvas (RGBA buffer, AA shapes, glow, blend), font (5x7),
                  wrap, fit_scale; examples/sheet.rs draws a test sheet
crates/kit        the browser end: Screen (buffer -> canvas, pixel scale,
                  ui text scale), gl (WebGL2 + a pixel layer), input (keys,
                  fingers, mouse, pointer lock), Link (reconnects, Hello
                  first), Session (the key), Pointer, Version, Socket,
                  TextField, storage, audio
crates/gpu        WebGPU device (wgpu), Caps, the pixel layer
crates/render     the engine: retained scene, geo, sculpt, terrain, shadows,
                  HDR + AO + bloom + ACES, grass, sea, light grid;
                  crates/showcase-web is its test page (/showcase/)
crates/hub-web    the front page (cards, live counts, scroll, footer), watch
                  (wyrm's card, live, as a watcher), wand (Wandfall's)
crates/wyrm       the game: laws world bots grid proto view mirror room
crates/wyrm-look  how it looks: ground, food, snakes, bursts (page + hub)
crates/wyrm-web   its page: lib (input, socket) state render (HUD) menu
crates/luciphon*  game #2, legacy: core, -look (2D), -web (WebGL2 3D)
crates/wandfall   game #3's core: laws trig map places motion storm world bots
                  predict proto view room (spec: docs/plunder.md)
crates/wandfall-look  its look (page + hub): look land aura rig/ fx/
                  state icon scene spectate camera
crates/wandfall-web  its page: page/ bar hud menu sound touch
crates/server     main (routes) host (a Room's thread; panics rebuild it)
                  store (snapshots) souls (names) signal (SIGTERM)
web/index.html    the hub page; web/<game>/index.html each game's page
scripts/          build-web.sh (dist/: hub at /, games at /<id>/),
                  ship.sh (ship/: image for Railway), caps.sh
```

Server routes: `/ws/<room>` a game (`/ws` and `/ws/arena` are wyrm, for
old pages; `?watch=1` only looks), `/ws/hub` the live Stats once a second,
`/health` (`ok <build> ...`), `/stats` (JSON); with `--static dist` the pages (`/arena`
redirects to `/wyrm/`, as `web/vercel.json` does). A page's first connection carries
`?v=1` and counts a visit; visits persist in `$DATA_DIR/visits` (the image
sets `/data`; a volume there keeps them across deploys), as are `souls`
and `rooms/<id>/snap-*.bin`. A deploy is a Stillness: SIGTERM, each room
`still()`s and saves, pages keep their picture and resume on reconnect.
CI ships the server only when its build hash differs from live /health.

Pixels: `kit::Screen` makes a buffer pixel `scale` CSS pixels (~960 across
at most), shown sharp; `ui()` is the text scale that reads the same on any
screen (2 phone, 1 desktop).

## A new game, from the template

1. Copy `crates/wyrm`, `crates/wyrm-look` and `crates/wyrm-web` to
   `crates/<id>`, `<id>-look`, `<id>-web`; rename packages; its world,
   wire, view and look are yours.
2. Its `room.rs` implements `engine::room::Room` with `id() == "<id>"`
   (watchers: see wyrm's); add it to `rooms` in `crates/server/src/main.rs`.
3. Its page: `web/<id>/index.html` (copy wyrm's), a `page` line in
   `scripts/build-web.sh`, a card in `crates/hub-web/src/lib.rs` `CARDS`
   (a live preview like `watch.rs`, or a still one), and its pkg path in
   `web/vercel.json`.

## Commands

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p secretspace-hub -p secretspace-wyrm-web -p secretspace-luciphon-web -p secretspace-showcase-web -p secretspace-wandfall-web --target wasm32-unknown-unknown -- -D warnings
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
- New people are ghosts for `GHOST_TICKS`: they cannot die or kill
  (without it, test players died in 8 s).
- Bodies are rebuilt into the grid after deaths and before spawning; a
  stale grid indexes snakes that are gone.
- The page steers by the pointer's angle from the screen's centre, where
  the camera keeps your head.
- A ring or `outside_circle` as big as the arena shades only on-screen
  pixels; skip it when its edge is out of view.
- Text sized for a desktop overflows a 390-px phone: wrap it (`wrap`) or
  fit it (`fit_scale`), and check the phone screenshot.
