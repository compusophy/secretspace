# CLAUDE.md: secretspace

Read this first: the operating map. README.md is the pitch, the rules and
the lineage.

## What this is

A world made of browser tabs. Each tab holds one island; its four edges are
portals to strangers' islands (same browser: BroadcastChannel; other
devices: WebRTC introduced by the relay). Motes are minds in a total,
fuel-metered language whose fuel is their own balance; they eat light, breed,
mutate and cross portals. The sun is attention: document visibility.

## Constitution (scripts/caps.sh enforces what it can)

1. **`crates/space` has zero dependencies.** std only. The relay may depend
   on it; it depends on nothing.
2. **Deterministic world.** In `space` there are no floats in the physics,
   no HashMap/HashSet, no clocks and no randomness but `rng`. An island is
   a pure function of its id and its inputs (watched per tick, portal view,
   arrivals). Same inputs mean the same `hash()`.
3. **The ledger is the only money path.** Every erg moves through `Island`'s
   own acts; `conserved()` must hold after every tick and every release.
   Never move ergs around it.
4. **The physics never faults.** Capabilities return sentinels (0/-1), never
   errors. Compile failures are coded `E01xx` with line and column.
5. **Every law lives in `laws.rs`.** The physics card prints them; caps docs
   name their numbers, so keep them in step when a law changes.
6. **Untrusted bytes never panic.** `wire::decode` is bounded and fuzzed;
   the mind compiler is fuzzed. Keep both tests.
7. **Caps:** a source file holds at most 1,000 lines; `crates/space/src` at
   most 5,000; this file at most 8,000 characters. At a cap: split, shrink,
   or delete. Never raise one.
8. **wasm32 always green:** `cargo check -p secretspace-web --target
   wasm32-unknown-unknown`.

## Map

```
crates/space/src  mind caps genome island laws founders wire net rng hash
crates/space/tests mind.rs (language guarantees) world.rs (physics, wire, nets)
crates/space/examples run genomes release   headless experiments
crates/web/src    lib (wiring, panels) app (tab state) draw (canvas)
                  bus (BroadcastChannel) mesh (WebRTC) place store
crates/relay/src  main (http, ws session, static files) hub (introduce,
                  forward, census sum) ws (handshake, frames, sha1, base64)
web/index.html    the page: canvas + two panels + a one-line bootstrap
scripts/          build-web.sh (dist/), caps.sh
```

Tick order (`Island::step`): sun eases toward watched, arrivals land, light
is minted (dithered hundredths), every mote that existed before the loop
thinks (newborns wait), then settle (departures exported, the starved and old
composted, occupancy rebuilt).

Page loop: a 50 ms interval pumps due ticks (100 ms apart; 2 s apart in
full dark; up to 600 caught up after a throttled timer), then flushes
envelopes to the bus, the mesh, and the relay (census only). rAF only draws.

## Commands

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p secretspace-web --target wasm32-unknown-unknown -- -D warnings
cargo fmt --all --check
bash scripts/caps.sh
bash scripts/build-web.sh          # needs wasm-bindgen-cli = Cargo.lock's wasm-bindgen
cargo run -p secretspace-relay --release -- --static dist   # :8787, page + relay
```

Browser checks: Playwright with `executablePath` at the preinstalled
Chromium. Separate *contexts* behave as separate devices (no shared
BroadcastChannel), so they test the WebRTC path; launch with
`--disable-features=WebRtcHideLocalIpsWithMdns` so local candidates resolve.
Fake dusk by redefining `document.visibilityState` and dispatching
`visibilitychange`.

## Gotchas (each cost a debugging session)

- **The cheapest mind that breeds wins a full island.** Left alone, founders
  evolve down to `harvest()` plus a spawn line within ~2,000 ticks, so most
  of a lineage's motes can be sterile minimal variants. That is the physics
  working; judge minds on a world with night in it.
- **A lone newcomer is lost to drift.** No single released mote, not even a
  copy of the dominant genome, took hold on a mature island (0/18). Hence
  release = a clutch of 8 on ground cleared for it, endowment drawn from the
  clearing's light first. `examples/release` measures it; the template must
  stay viable (`the_template_can_take_hold`).
- **Release must not starve its own newborns.** Drawing the endowment from
  the cells around the spot left them in a desert.
- **Night was too lethal at day speed:** ~95% died within 15 s of a tab
  being hidden. Full dark now ticks 20x slower; a lit, empty island
  re-runs genesis after 100 ticks.
- **Thinking costs dominate.** A 60-step mind cannot live on a mature
  island's ~30 ergs a tick. Before blaming the code, check `last_used` (the
  inspector shows it).
- **`pkill -f relay` kills your own shell** when the command line contains
  the pattern. Track the relay's PID instead.
- Test harnesses that call `simulate` repeatedly must carry one clock
  across calls; restarting at 0 makes timeouts never fire.
- `Net` accepts envelopes only from islands it has heard greet (`Hello`).
  Dedupe is per peer by sequence number; the same island heard over both
  the bus and the mesh is harmless.
