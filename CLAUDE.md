# CLAUDE.md: secretspace

Read this first: the operating map. README.md is the pitch.

## What this is

A hub of tiny multiplayer games, all Rust. The front page (`/`) shows a
card per game with live player counts; its footer, who is online and all
visits ever. Each game is
its own page (`/wyrm/`, ...) talking to its own room on one server.

The first game, **wyrm** (snakes: eat the glow, grow, make them run into
you), is the template the next ones are cloned from: an authoritative
world on the server, each browser sent only what changed in its view, and
a page that draws every pixel itself into one buffer. Its hub card is the
real game, live: the hub watches the room (`?watch=1`, never counted) and
draws it in the game's own look.

Game #2, **Luciphon** (`docs/luciphon.md`), is **legacy, frozen** (live,
card hidden). **Now: the engine** (`docs/engine.md`,
UE5-class WebGPU rendering for every game) and game #3, a wand battle
royale like Plunderstorm (`docs/plunder.md`), its first consumer.

## Rules

1. **No hand-written JavaScript or TypeScript.** Pages are a canvas and one
   line that starts the wasm (wasm-bindgen writes its own glue). Every
   pixel is drawn by `pixels` into a buffer and shown once a frame, except
   3D: WebGPU (`gpu`, WGSL) or WebGL2 (`kit::gl`, GLSL), shaders as Rust
   strings, with a `pixels` HUD layer over it. The only DOM besides the
   canvas: `kit::TextField` (an invisible input, so phones offer their
   keyboard) and the frame the hub opens a game in (`kit::shell`).
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
7. **wasm32 always green:** the wasm clippy in Commands. Every WGSL
   string has a naga test; Wandfall's predicted code calls `crate::trig`,
   never platform floats (`caps.sh` checks both).

## Map

```
crates/engine     wire rng room (Room trait) hub (Stats) who (Hello, Seen,
                  names) words snap (save files) fixed sha1 synth (sounds)
crates/pixels     Canvas (RGBA buffer, AA shapes, glow), font (5x7), wrap,
                  fit_scale; examples/sheet.rs (a test sheet)
crates/kit        the browser end: Screen, place/snap/on_resize (whole
                  device pixels), gl (WebGL2), input (keys, fingers, pointer
                  lock), Link (reconnects, Hello first), Session, Version,
                  TextField, storage, audio, meta (every game's Esc menu),
                  shell (games open over the hub, full screen), report
crates/gpu        WebGPU device (wgpu), Caps, Health, the pixel layer
crates/render     the engine: retained scene, geo, sculpt, terrain, shadows
                  (cascades + island layer), HDR + AO + bloom + ACES, grass,
                  sea, decals, slots (bind tables), light grid; its test page
                  crates/showcase-web (/showcase/)
crates/hub-web    the front page: shelf (layout) backdrop tag; watch (wyrm's
                  card, live), wand (Wandfall's), desk (Battlestation's)
crates/wyrm       the game: laws world bots grid proto view mirror room
crates/wyrm-look  how it looks: ground, food, snake, bursts, live (page + hub)
crates/wyrm-web   its page: lib (input, socket) state render (HUD) menu
crates/luciphon*  game #2, legacy: core, -look (2D), -web (WebGL2 3D)
crates/wandfall   game #3's core: laws trig map places motion/ (run jump
                  ledge collide) storm world/ bots/ spells loot hall predict
                  proto view room (spec: docs/plunder.md)
crates/wandfall-look  its look (page + hub): look land/ flora basalt aura
                  make rig/ fx/ state icon scene spectate camera sky
crates/wandfall-web  its page: page/ hud/ menu/ bar sound ambience steps
                  touch lessons reload settings
crates/battlestation*  desk sim: laws keys hands term; -look: gear scene
                  body glass monitor light desk; -web: page/ (os.rs:
                  compusophyOS, so pages send COOP+COEP)
crates/server     main (routes) http sockets host (a Room's thread; panics
                  rebuild it) store souls signal (SIGTERM) feedback
web/index.html    the hub page; web/<game>/index.html each game's page
scripts/          build-web.sh (dist/: hub at /, games at /<id>/), ship.sh
                  (ship/: image for Railway), caps.sh (+ shaders.awk)
```

Server routes: `/ws/<room>` a game (`/ws` and `/ws/arena` are wyrm, for
old pages; `?watch=1` only looks), `/ws/hub` the live Stats once a second,
`/health` (`ok <build> ...`), `/stats` (JSON), `/feedback` (POST a report;
GET `?key=$FEEDBACK_KEY` reads them); with `--static dist` the pages (`/arena` ->
`/wyrm/`, as in `web/vercel.json`). A page's first connection carries
`?v=1` and counts a visit. `$DATA_DIR` (`/data` in the image, a volume)
keeps `visits`, `souls`, `feedback`, `rooms/<id>/snap-*.bin`. A deploy is
a Stillness: SIGTERM, rooms `still()` and save, pages resume after.
CI ships the server only when its build hash differs from live /health.

Pixels: `kit::Screen` makes a buffer pixel `scale` CSS pixels (~960
wide at most); `ui()` is the text scale (2 phone, 1 desktop).

## A new game, from the template

1. Copy `crates/wyrm`, `-look`, `-web` to `crates/<id>`, `<id>-look`,
   `<id>-web`; rename packages; world, wire, view and look are yours.
2. Its `room.rs` implements `engine::room::Room` (`id() == "<id>"`;
   watchers: see wyrm's); add it to `ROOMS` in `crates/server/src/main.rs`.
3. Its page: `web/<id>/index.html` (copy wyrm's), a `page` line in
   `scripts/build-web.sh`, a hub-web `CARDS` card (live like `watch.rs`,
   or still), its pkg path in `web/vercel.json`; Esc opens `kit::meta`.

## Commands

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p secretspace-hub -p secretspace-wyrm-web -p secretspace-luciphon-web -p secretspace-showcase-web -p secretspace-wandfall-web -p secretspace-battlestation-web --target wasm32-unknown-unknown -- -D warnings
cargo fmt --all --check
bash scripts/caps.sh
bash scripts/build-web.sh   # needs wasm-bindgen-cli = Cargo.lock's wasm-bindgen
cargo run -p secretspace-server --profile server -- --static dist   # :8787
```

A push to main or a `claude/` branch deploys (`deploy.yml`): tests, the
server to Railway (`railway up` of `ship/`; RAILWAY_TOKEN, _SERVICE), the
pages to Vercel (VERCEL_*) pointed at it by RELAY (`wss://<railway>/ws`;
none, no page deploy). One production: merge live branches first.

Browser checks: Playwright on the installed Chromium; a context a player;
`hasTouch, isMobile, deviceScaleFactor: 3` at 390x844 a phone.

## Gotchas

- **`pkill -f server` kills your shell** when the command line holds
  the pattern. Track its PID.
- Worktrees sharing a CARGO_TARGET_DIR run each other's code (path crates
  are fingerprinted by relative path): one target dir per worktree.
- New people are ghosts for `GHOST_TICKS` (else test players die in 8 s).
- Bodies are rebuilt into the grid after deaths and before spawning; a
  stale grid indexes snakes that are gone.
- The page steers by the pointer's angle from the screen's centre, where
  the camera keeps your head.
- Text sized for a desktop overflows a 390-px phone: wrap it (`wrap`) or
  fit it (`fit_scale`); check a phone shot.
