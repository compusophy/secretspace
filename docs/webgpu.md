# Luciphon on WebGPU: toward UE5-class rendering in the browser

**For:** the local agent (full repo access, Chrome with Claude, parallel
subagents). **From:** the cloud session that built Luciphon's first-person
3D, the wand, gear and monsters (October 2026). **Owner's ask, verbatim:**
"yes lets try for near unreal5 feature parity with webgpu".

Read `CLAUDE.md` first: its rules bind this work. This plan says what to build,
in what order, how to prove each step, and how to test it massively in
parallel. Tick the boxes as you go, and keep the notes short and measured.

---

## 0. The goal, and what "parity" means here

Unreal Engine 5 itself is out. It has no official web target, ports are
third-party, the downloads are huge, and it would replace our Rust stack.
The decision and its sources are in the cloud session's answer to the owner.
"Near UE5 parity" means **the look and feel of UE5's headline features**, built
in Rust on WebGPU, in a page that still loads in seconds:

| UE5 feature | Our web equivalent | Phase |
|---|---|---|
| Lumen (dynamic GI) | Irradiance probes (DDGI-lite) updated by compute, plus emissive surfaces as lights, plus SSGI/GTAO | 7 |
| Nanite | GPU-driven rendering: cluster culling in compute (frustum and Hi-Z occlusion), indirect draws, LOD chains | 6 |
| Virtual shadow maps | Cascaded shadow maps from the moon (3-4 cascades, PCSS), and a point-shadow atlas for the nearest lights | 3 |
| Many dynamic lights | Clustered (froxel) forward+ lighting: thousands of lights (every lantern, crystal, Lumen, mote and beam) | 3 |
| Materials | PBR (GGX, metal/rough), procedural triplanar detail and normal maps, image-based light from a sky cubemap built at runtime | 2 |
| Post | HDR (rgba16float), physically based bloom, auto exposure (histogram), AgX/ACES tone map, colour grading, vignette | 2 |
| TSR | TAA with motion vectors and jitter, then temporal upscaling (render at 0.5-0.8, output native) | 4 |
| Screen-space | GTAO, SSR (water, marble, glass), contact shadows | 4 |
| Volumetrics | Froxel volumetric fog with light shafts (god rays from the Luciphon's bell, lanterns, beams) | 5 |
| Niagara | GPU particles: compute simulation, indirect draw, soft particles, trails (beams, embers, fireflies, ambient motes) | 5 |
| Foliage | Instanced grass and flowers generated per tile on the GPU, wind sway, tree sway | 6 |
| Water | Reflective and refractive water tiles with foam and caustics | 4-5 |

**Hard limits of WebGPU** (design around them; do not fight them):
- No 64-bit atomics in core, so Nanite's software rasteriser cannot be ported
  as-is. Do the cluster culling and indirect draws; skip the visibility-buffer
  software raster.
- No bindless resources and no core multi-draw-indirect. Default
  `maxStorageBuffersPerShaderStage` is 8 and `maxBindGroups` is 4.
- Optional features vary by browser: `timestamp-query`, `float32-filterable`,
  `rg11b10ufloat-renderable`, `shader-f16`, subgroups,
  `indirect-first-instance`. Feature-detect each one and keep a path without it.
- Support (2026):
  - Chrome and Edge 113+: Windows, macOS, ChromeOS; Android 121+ on many GPUs;
    Linux only on recent Intel and NVIDIA.
  - Safari 26 on macOS, iOS and iPadOS.
  - Firefox on Windows and Apple Silicon Macs only.
  - **A fallback is mandatory** (§2.3).

**Success:**
- A desktop on Ultra looks like a UE5 night scene: moonlit, volumetric,
  thousands of small lights, soft shadows, a glowing bell with god rays.
- A mid-range phone holds 30+ fps on Medium.
- The fallback is today's WebGL2 renderer, unchanged.
- Gameplay, netcode and the server are untouched.

---

## 1. What exists today (read these files first)

**The page:** `crates/luciphon-web/src/`

| File | What it does |
|---|---|
| `lib.rs` | The frame loop: `frame` → net → chunk builds (2 a frame, nearest first) → `hands` (input, 30 Hz Inputs, prediction) → `draw` (builds a `scene::Frame`) → `overlay` (the HUD) → `present`. Also `camera()`: first person, the title orbit, the Underlight. |
| `scene/mod.rs` | `Scene` (programs, chunk meshes, unit meshes), `Camera`, `Frame`, `Hand`, `wand()`, `flame()`, `rarity()`. Draw order: sky → opaque chunks → things → transparent (ghosts, build ghost, node rings, beams) → additive sparks → hand (depth cleared). |
| `scene/chunk.rs` | One mesh per 32x32-tile chunk: ground quads (claim hue tint, uneven natural ground), cliffs to the void, rocky underside with stalactites, every object, and its static lights. |
| `scene/things.rs` | Procedural low-poly meshes: trees, rocks, crystals, moss, brambles, pillars, the Luciphon shrine and bell, hearth, walls, doors, lanterns, planters and crops; `lumen()` (robe and glow), `beast(kind)`, mote, glim, hand, cube, ring, rod; `light_of()`. |
| `scene/shapes.rs` | `Geo`: interleaved vertices (pos 3, normal 3, colour 3 + glow 1 = 10 floats), `tri`, `quad`, `column`, `block`, `blob`, `spike`. Flat shaded, wound outward (tested). |
| `scene/shaders.rs` | GLSL ES 3.0: the world shader (hemisphere ambient, moon, the 16 nearest point lights, glow, fog, the Underlight negative), sky (gradient and hashed stars), points (sparks). |
| `hud.rs`, `bag.rs`, `play.rs`, `first.rs` | The pixel HUD (crosshair, vitals, names, monster bars, markers, phone buttons, gear panel, wheel, title). Drawn into `kit::gl::Gl::hud` (a premultiplied `pixels::Canvas`), uploaded as a texture each frame. |
| `fx.rs` | CPU sparks and rings, shake, hit flash. `points()` feeds the sparks shader. |
| `state.rs` | The mirror, prediction, the drawn self (lerped between ticks), interpolation of others (66 ms behind), beams, chunk staleness, event feel. |
| `controls.rs`, `laws.rs` | Input and `Feel` (camera FOV, eye height, fog distances, max DPR 2 desktop and 1.5 touch). |

**Elsewhere:**
- `crates/kit/src/gl.rs`: the WebGL2 screen (`Gl`: canvas, context, HUD
  layer, `present`), `Program`, `Mesh`, `m4` maths (column-major).
- `crates/kit/src/input.rs`: keys, fingers, mouse, pointer lock.

**Conventions you must keep:**
- **Space.** World tile `(x, y)` at height `h` is `(x, h, y)` in render space:
  x east, y up, z south. Headings are `u16` (65536 a turn; 0 east, 16384
  south). Camera forward is `(cos yaw cos pitch, sin pitch, sin yaw cos pitch)`.
- **The island.**
  - 128x128 tiles in 4x4 chunks; the island's radius is 60.
  - Rings: Sanctum < 10 < Glow < 34 < Dim < 52 < Rim.
  - Ground kinds: marble, meadow, moss, dark, ice, mud, water, glass. The
    ground level bits exist but are unused (flat).
  - About 320k vertices for the whole island.
- **What the renderer is given each frame** (`scene::Frame`):
  - the camera;
  - `things` (Lumens, motes, pickups and beasts with interpolated x, y, z,
    facing and state bits);
  - the hand (hue, flame, swing, charge, perfect);
  - the build ghost, node rings, beams, CPU sparks, your light, the
    Underlight flag, the time and fog.
  - **Keep this interface.** The new renderer consumes the same `Frame`, so
    gameplay code does not change.
- **Do not touch** `crates/luciphon` (the shared core), `crates/server`, or
  the wire. A pure page change never redeploys the server (CI compares build
  hashes).

**Numbers today:**
- The Luciphon page's wasm is about 145 KB gzipped.
- CPU frame time is 1-3 ms (`?perf=1`).
- Headless SwiftShader renders it correctly.

---

## 2. Architecture

### 2.1 wgpu, behind a quality switch

Use **`wgpu`** (Rust; WebGPU on the web, Vulkan/Metal/DX12 natively):
- It gives native rendering for fast iteration and golden-image tests in
  `cargo test`.
- It gives Naga validation of every WGSL shader in unit tests.
- It shares one codebase with any future native client.
- WGSL lives in Rust strings, as the GLSL does today (rule 1).

**Version coupling (do this first):** `Cargo.lock` pins `wasm-bindgen =
0.2.129` (see `crates/*/Cargo.toml`).
- Pick the newest `wgpu` whose web backend works with it, or bump
  wasm-bindgen workspace-wide.
- CI installs the wasm-bindgen CLI version from `Cargo.lock` automatically
  (the "wasm-bindgen CLI version Cargo.lock pins" step), and so does
  `scripts/build-web.sh`'s requirement. Verify both still pass after a bump.
- Async device creation needs `wasm-bindgen-futures`.

**Escape hatch:** if wgpu's wasm costs more than ~1.5 MB gzipped (measure in
Phase 0), consider raw WebGPU through `web-sys`
(`--cfg=web_sys_unstable_apis`). Record the decision in this file.

### 2.2 Crates and modules

```
crates/gpu            NEW. Generic: device/adapter/surface setup (web +
                      native), feature detection -> Caps, the frame's render
                      targets, a tiny pass graph, buffer/texture helpers,
                      pipeline cache with async creation, the HUD layer
                      upload+composite (premultiplied, NEAREST), GPU
                      timestamps when available. No game knowledge.
crates/luciphon-web/src/render/   NEW. Luciphon's renderer on `gpu`:
    mod.rs        Renderer: consumes scene::Frame, owns passes and tiers
    world.rs      chunk meshes and instanced objects (from scene/chunk.rs,
                  scene/things.rs geometry: reuse Geo, add tangents/material ids)
    lights.rs     light list, clustered culling (compute)
    shadow.rs     cascades + point-shadow atlas
    post.rs       HDR, bloom, exposure, tone map, grade, TAA/TAAU
    ssao.rs ssr.rs fog.rs particles.rs grass.rs water.rs gi.rs sky.rs
    wgsl/*.rs     shader sources as consts, one file per shader family
crates/luciphon-web/src/scene/    KEPT: the WebGL2 renderer, frozen, as the
                  fallback tier. Geometry builders (shapes, things, chunk)
                  are shared by both renderers.
```

Every file stays under 1,000 lines (rule 6): split early.

### 2.3 Tiers and the fallback

| Tier | Who | What |
|---|---|---|
| **GL** | No `navigator.gpu`, or adapter or device creation fails | Today's WebGL2 renderer, unchanged |
| **Low** | WebGPU, weak adapter or phone | HDR, tone map, bloom, clustered lights, 1 shadow cascade, render scale 0.6 + TAAU, no SSR, no GI, no volumetrics |
| **Medium** | Phones that hold it, iGPUs | Low, plus 2 cascades, GTAO (half res), cheap froxel fog, GPU particles, grass near only |
| **High** | Desktop GPUs | Medium, plus 3 cascades with PCSS, SSR, full froxel fog and god rays, grass to 30 tiles, point shadows (4) |
| **Ultra** | Strong desktops | High, plus DDGI probes, 4 cascades, point shadows (8), native-res TAA, more grass, contact shadows |

- **Choosing a tier:**
  - Start from the adapter: `navigator.gpu` and adapter info or limits.
  - Then measure the first 120 frames. Step down a tier if the 95th-percentile
    frame time exceeds budget, and step up after 10 s comfortably under it.
  - `?q=gl|low|medium|high|ultra` forces a tier; `?gpu=0` forces GL.
- **The tier's numbers** live in `luciphon-web/src/laws.rs` (rule 3), as a
  `Look` table next to `Feel`.
- Add a small "graphics" row to the gear panel (`bag.rs`) to pick a tier;
  remember it in localStorage.

### 2.4 Determinism hooks for testing (build these in Phase 0)

| Hook | What it does |
|---|---|
| `?cam=x,y,h,yaw,pitch` | Places a free debug camera (no player needed; title-screen state is fine). |
| `?t=ms` | Freezes render time (sky twinkle, bobbing, fog animation, particles) at that moment. |
| `?hud=0` | Hides the HUD. |
| `?shot=1` | All three above, plus a fixed resolution and render scale 1. Once frame N has fully rendered, sets `document.title = "shot ready"`. |
| `?perf=1` (exists) | Extend the title line with tier, render scale, CPU and GPU ms (timestamp-query when available), draw calls, lights after culling, triangles. Keep the existing tokens (`@x,y yaw ... beast d,a`): the test bots steer by them. |

---

## 3. Phases

Every phase:
1. Keep all gates green (`CLAUDE.md` § Commands).
2. Run the parallel test sweep (§4) before pushing.
3. Post before/after screenshots and perf numbers under the phase in this
   file.
4. Push to a `claude/` branch (it deploys; see `CLAUDE.md`).
5. Each phase is shippable: tiers keep anything unfinished off.

### Phase 0: spike and plumbing (1-2 days)
- [ ] `crates/gpu`: adapter and device on web and native, `Caps` (features,
      limits), a surface sized like `kit::gl::Gl` (DPR caps from `Feel`), and
      the HUD layer composite (port `Gl::present`: premultiplied, NEAREST,
      the layer slightly larger than the window).
- [ ] The page picks a renderer at start (§2.3); GL stays the default until
      Phase 1 passes.
- [ ] §2.4 hooks.
- [ ] Measure and record: wasm size (gzipped) with wgpu, time to first
      frame, device creation time on Chrome, Safari and Firefox.
- [ ] A native test renders an empty frame offscreen (skip if no adapter),
      and every WGSL string passes Naga validation.

### Phase 1: parity port (2-4 days)
- [ ] The current scene on wgpu, the same look:
  - chunks;
  - things: Lumens (robe and glow), beasts, motes and glim;
  - the sky, sparks, beams, rings, the build ghost, the hand and wand;
  - fog and the Underlight negative.
- [ ] Golden images: 12 fixed `?shot` poses (§4.2), GL vs WebGPU, diff under
      2% of pixels (sky stars excepted).
- [ ] Perf no worse than GL on the same machine. Then WebGPU becomes the
      default where available.

### Phase 2: HDR, PBR, post (3-5 days)
- [ ] An `rgba16float` scene target, with physically based bloom (dual-filter
      downsample and upsample, energy-conserving).
- [ ] Auto exposure: a compute histogram, then an eased average.
- [ ] AgX (or ACES) tone map, then a grading LUT (night: teal shadows, gold
      highlights; the Underlight: inverted and cold). Vignette and slight
      chromatic aberration on High and Ultra.
- [ ] PBR (GGX, metal/rough, energy compensation):
  - Material ids per vertex: marble, wood, stone, foliage, crystal, glass,
    water, cloth (robes), metal (the bell, lantern), emissive.
  - Procedural detail via triplanar noise normals, generated once at load by
    compute into small textures. **No downloaded textures.**
- [ ] IBL: render the sky into a cubemap at load and when the sky changes;
      prefilter (GGX) and an SH ambient term.
- [ ] Emissive surfaces (bell, lanterns, crystals, eyes, beams) push into
      bloom: the bell should *bloom*.

### Phase 3: lights and shadows (4-6 days)
- [ ] Clustered forward+:
  - Froxels (16x9x24), with a compute pass assigning lights per cluster.
  - A light SSBO for every light: static ones from chunks, plus dynamic ones
    (Lumens, motes, glim, beams as short-lived line lights, beasts' eyes and
    the wraith's core, hits).
  - Target 2,000 lights at under 1.5 ms on High.
- [ ] Moon cascaded shadow maps:
  - 3-4 cascades with stable snapping and PCF.
  - PCSS on High and Ultra.
  - Cache static-geometry cascades and re-render only dynamic casters (a
    huge win: the island is static).
- [ ] A point-shadow atlas (cube faces as atlas tiles) for the N most
      important lights (the bell first, then the nearest lanterns and
      hearths). Static casters cached as above.
- [ ] Contact shadows (screen-space) for small detail on Ultra.

### Phase 4: temporal and screen-space (4-6 days)
- [ ] Motion vectors (camera plus per-object: Lumens and beasts carry
      previous transforms; chunks are static).
- [ ] TAA (jitter, neighbourhood clamping, history rejection on disocclusion),
      then TAAU (render scale 0.5-0.8 per tier). Like UE's TSR, this is what
      makes phones possible.
- [ ] GTAO at half res with a bilateral upsample.
- [ ] SSR: hierarchical depth march on water, marble, glass and ice (from
      material roughness). Fall back to IBL.
- [ ] Water tiles get their own shader:
  - lowered surface (−0.06), normal-mapped ripples;
  - SSR reflection and refraction of the depth below;
  - foam at shores, caustics on nearby ground.

### Phase 5: atmosphere and particles (4-6 days)
- [ ] Froxel volumetric fog:
  - Height-and-ring-varying density (the Dim is thicker, the Rim misty).
  - In-scattering from clustered lights and the moon, with shadowed shafts
    from cascades and point shadows. God rays from the bell.
  - Temporal reprojection.
  - The current distance fog becomes this on Medium and up.
- [ ] GPU particles:
  - Compute simulation, append/consume buffers, indirect draw, soft
    particles (depth fade), curl-noise motion.
  - Systems: hit sparks, beam trails and impact bursts, embers over hearths,
    fireflies in the Glow, drifting motes in the Dim, the wraith's dark
    wisps, the Felled burst, kindling waves, rain or mist (optional).
  - `fx.rs` keeps the gameplay feel; the GPU system renders it plus ambient
    life.
- [ ] Beams become volumetric-looking: a core line, a glow sprite chain, a
      light, scorch decals where they hit.

### Phase 6: GPU-driven geometry, foliage, animation (5-8 days)
- [ ] Objects become instanced:
  - One mesh per object kind (with 2-3 LODs from a simple Rust vertex-
    clustering or quadric simplifier).
  - Per-instance transform, tint, hue and material.
  - Chunk meshes keep only ground, cliffs and underside.
- [ ] Compute culling:
  - Frustum and Hi-Z occlusion (last frame's depth pyramid), writing
    indirect args per mesh/LOD.
  - With `indirect-first-instance` where available; a fallback path without
    it.
- [ ] Grass and flowers:
  - Generated per tile by compute from ground kind and a hash: meadow and
    moss dense, marble none.
  - Wind (noise field) and bending away from nearby Lumens and beasts.
  - Distance-faded.
- [ ] Tree sway and cloth: robe and hood sway in the vertex shader, driven by
      velocity (from interpolated positions).
- [ ] Beast and Lumen animation, procedural in shaders:
  - legs for gloomhounds, bob and lean for hushlings, float and tatter for
    wraiths;
  - the wind-up pose (crouch, eyes flare).
- [ ] Terrain: use the tile `level` bits if the core ever sets them; until
      then, gentle vertex displacement on natural ground and real rock
      geometry on cliffs (noise-displaced, LOD by distance).

### Phase 7: global illumination (5-10 days, Ultra and High only)
- [ ] DDGI-lite:
  - A probe grid over the island (1 probe per 2-4 tiles, 3-4 heights).
  - Each frame, update a budgeted subset: ray-march a coarse voxel or SDF of
    the island (built once per chunk at chunk-build time; updated on tile
    changes), with irradiance and visibility in octahedral textures.
  - Sample in the world shader with probe-visibility weighting.
  - Static lights bake into the probes; dynamic ones stay in the clustered
    pass.
- [ ] Emissive surfaces feed GI: the bell lights the shrine, lanterns light
      their walls, crystal fields glow teal on the rocks around them.
- [ ] SSGI (screen-space) on Ultra for small-scale bounce. Reflections
      sample probes beyond SSR.

### Phase 8: tune, phones, polish (ongoing)
- [ ] Budgets (§5) on the test matrix (§4.4); automatic tier stepping proven.
- [ ] Shader compile stalls:
  - Create every pipeline async at load, behind the title screen.
  - Show a "lighting the island…" progress line (the island already turns
    behind the title).
- [ ] Device loss: recreate everything from `Frame` data (chunks rebuild
      from the mirror's tiles), and never crash the page.
- [ ] Memory: release chunk resources on `ChunkGone`. Keep the GPU memory
      estimate under 300 MB on Medium and 150 MB on Low.

---

## 4. Testing: massively parallel, with Chrome and subagents

The orchestrating agent runs a **sweep** after every meaningful change.
Subagents run in parallel, each in its own Chrome window or tab (separate
contexts are separate players). Each writes a short report:

```
docs/webgpu-runs/<date>-<phase>/<role>.md
```

The report holds screenshots, numbers, failures and repro URLs.
**Commit the reports. Do not commit the screenshots.**
- Small PNGs (under 200 KB) may go in `docs/webgpu-runs/` when they show a
  bug or a milestone.
- Everything else stays local.

### 4.1 Local setup
- `bash scripts/build-web.sh && cargo run -p secretspace-server --release --
  --static dist` serves everything on `:8787`. Use `PORT=...` to run several
  servers if needed.
- **Real GPU tests:** use Chrome with Claude (headed, real WebGPU).
- **Headless:** Playwright with the preinstalled Chromium.
  - The flags that worked for WebGL2 in the cloud: `--use-gl=angle
    --use-angle=swiftshader --enable-unsafe-swiftshader
    --ignore-gpu-blocklist`.
  - For WebGPU, try `--enable-unsafe-webgpu` (plus Vulkan/SwiftShader flags).
    Check `await navigator.gpu?.requestAdapter()`.
  - If headless gets no adapter, use headless only for the GL fallback and
    golden GL images, and headed Chrome for WebGPU.
- **Pointer lock and mouse look under Playwright:** moves must be
  **cumulative** (`mx += d; mouse.move(mx, y)`). Moving out and back nets
  zero `movementX`.
- **Steering bots:** read `document.title` under `?perf=1` for position, yaw
  and the nearest beast (`@x,y yaw Y beast dist,angle`).
- The cloud session's scripts are good starting points (recreate locally;
  they lived in a scratchpad):
  - a walker that steers outward by position until it falls off the Rim;
  - a hunter that turns toward the nearest beast and fires;
  - a phone test using CDP `Input.dispatchTouchEvent` for twin thumbs;
  - a gear-panel test (Tab, the phone's bag button).

### 4.2 Golden poses (for `?shot=1`)

Run each pose on GL and on every WebGPU tier. Save as
`shot-<pose>-<tier>.png`.

1. The Sanctum: the bell, looking up from 3 tiles.
2. The Sanctum from the Glow's edge, the whole shrine.
3. A birch grove in the Glow.
4. Ice and water tiles (reflections).
5. A crystal field in the Dim (emissive, GI).
6. The Rim at the island's edge, looking out and down (cliffs, underside,
   the Dark).
7. Looking straight up (sky, stars).
8. Looking straight down from 15 up (the Underlight's view, with `?under=1`
   if you add it).
9. A built hearth with walls and lanterns (spawn them via a test world or a
   save).
10. A crowd: 8 residents and 6 beasts in view.
11. Max particles: 20 beams in flight (a test hook or many players).
12. Phone portrait 390x844, DPR 3, on the Medium tier.

A diff tool (Node `pixelmatch` in the test harness is fine; rule 1 governs
shipped pages, not test harnesses) reports the percentage of changed
pixels against the previous accepted images.

### 4.3 Subagent roles (run all in parallel)

| Role | What it does | Pass |
|---|---|---|
| **Golden** | All §4.2 poses on GL and every tier; diff against accepted | No unexplained diff over 2%; new effects reviewed and accepted |
| **Perf** | Each tier, 60 s at 3 fixed routes (Sanctum, Dim fight, Rim edge); `?perf=1` CPU and GPU ms, p50/p95/p99 | Within §5 budgets |
| **Players x4-8** | Separate contexts join, fight in the Dim, craft, wear gear, fall into the Underlight, respawn | No page errors; prediction offsets stay small (watch for snaps); frame time steady |
| **Phone** | 390x844 DPR 3 touch: stick, look, buttons, gear panel; Medium and Low | Playable; text fits; 30 fps budget on a real phone when available |
| **Fallback** | `?gpu=0` and a browser without WebGPU (Firefox on Linux) | Identical to today's GL game |
| **Soak** | 30 min of play with tiers switching; chunks loaded and gone; tab hidden and shown | No GPU memory growth over 10%; no device loss, or recovery within 1 s |
| **Stillness** | Restart the server mid-play (the deploy path) | The picture holds, then resumes; no renderer errors |
| **Browsers** | Chrome, Edge, Safari 26 (macOS/iOS if available), Firefox (Windows/macOS) | Each tier works or steps down cleanly; record quirks |

### 4.4 Device matrix (as available locally)
- A desktop with a discrete GPU.
- A laptop iGPU.
- An Apple Silicon Mac (Safari and Chrome).
- An Android phone (Chrome).
- An iPhone (Safari 26).
- **Any you lack:** emulate viewport and DPR, and note the gap in the report.

---

## 5. Budgets

| | Ultra | High | Medium | Low | GL |
|---|---|---|---|---|---|
| GPU ms (p95) | 12 | 8 | 14 | 16 | as today |
| Render scale | 1.0 | 0.8-1.0 | 0.66 | 0.5 | DPR cap |
| Lights after culling | 4,000 | 2,000 | 512 | 256 | 16 |
| Shadow cascades | 4 | 3 | 2 | 1 | 0 |

- **Targets per machine:**
  - Ultra and High: a mid-range desktop (RTX 3060 / M1 class) at 1440p.
  - Medium: a mid-range phone at DPR 1.5.
  - Low: an old phone at 30 fps.
- **Page size:** the Luciphon wasm at most **1.5 MB gzipped** (today 145
  KB). Shaders are strings in it; there are no texture downloads.
- **Load:** the title screen within 3 s on broadband; pipelines ready
  within 6 s; never a blank page (GL draws while WebGPU warms up).

---

## 6. Risks

| Risk | Mitigation |
|---|---|
| wgpu ↔ wasm-bindgen version clash | Phase 0 settles it; bump workspace-wide if needed (CI and `build-web.sh` follow `Cargo.lock`) |
| wasm size balloon | Measure every phase; feature-gate tiers at compile time if needed; the escape hatch is raw web-sys WebGPU (§2.1) |
| Safari/Firefox WebGPU quirks | Feature detection everywhere; the Browsers role every phase; step down a tier, never crash |
| Shader compile hitches mid-play | All pipelines async at load; no new pipelines during play |
| Phones overheating or throttling | Tier stepping on p95; render scale and TAAU first; GI and volumetrics off on phones |
| Changing the game by accident | The renderer only consumes `scene::Frame`; core, server and wire untouched (the server's build hash stays the same; CI will show "server unchanged") |
| HUD regressions | The HUD stays a `pixels` layer composited last, exactly as now; the Golden role covers it |

---

## 7. Working rules for this effort
- All of `CLAUDE.md` applies:
  - Rust only in pages; files under 1,000 lines; tuning numbers in `laws.rs`
    (renderer numbers in a `Look` table in `luciphon-web/src/laws.rs`).
  - wasm clippy green; fmt; caps.
- Small commits, one effect at a time, each behind its tier. Push to a
  `claude/` branch to deploy, then verify live (`/health`,
  `/luciphon/?perf=1`).
- After each phase, update `CLAUDE.md`'s map (kept under 8,000 characters)
  and this file's checkboxes with measured notes.
- **Ask the owner** before:
  - raising the page-size budget;
  - adding any downloaded asset (textures, models);
  - changing gameplay feel (camera FOV, eye height, fog that hides enemies).

## 8. Open questions for the owner (answer when convenient; defaults in brackets)
1. Day and night: should the island get a day cycle with a sun (big for
   shadows and GI), or stay eternal moonlit night? [Night; a dawn on the
   Sanctum later.]
2. Art direction: stylised low-poly with UE5 lighting (the plan), or push
   toward realism with downloaded textures? [Stylised, procedural, no
   downloads.]
3. Minimum device: is a 2020-era phone a target? [Yes, on Low.]
