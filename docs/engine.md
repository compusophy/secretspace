# The engine: next-gen graphics and netcode in the browser, for every game

**For:** the local agent (full repo access, Chrome with Claude, parallel
subagents). **From:** the cloud session that built Luciphon's first-person
3D, the wand, gear and monsters (October 2026).

**The owner's direction, verbatim:** "we ned the rener engine solid first and
thats more important because we dont know what we need yet ... this will be
the foundation for all future games, not just one game, and our core
offering, majestic ue5 lervewl graphics in the browser with utlra high
performance on network and graphics".

**What this means:**
- **The engine is the product.** It is game-agnostic Rust, built on WebGPU,
  with a WebGL2 fallback.
- **Its consumers:**
  - Luciphon is the first consumer and the test bed.
  - A standalone showcase page proves the look.
  - The next game sets the requirements: a wand-based battle royale shooter
    in the spirit of Blizzard's Plunderstorm (see `docs/plunder.md`; not to
    be built yet).
- **The MMO is shelved.** Luciphon's gameplay stays as it is.

Read `CLAUDE.md` first: its rules bind this work. Tick the boxes as you go,
with short measured notes.

---

## 0. The goal

Unreal Engine 5 itself is out. It has no official web target, the ports are
third-party, the downloads are huge, and it would replace our Rust stack and
the code the server and the page share. "UE5-level" here means **the look of
UE5's headline features**, built in Rust on WebGPU, in pages that still load
in seconds and hold their frame rate. The netcode must suit a fast
first-person shooter.

| UE5 feature | Our web equivalent | Phase |
|---|---|---|
| Lumen (dynamic GI) | Irradiance probes (DDGI-lite) updated by compute, plus emissive surfaces as lights, plus SSGI/GTAO | 8 |
| Nanite | GPU-driven rendering: cluster culling in compute (frustum and Hi-Z occlusion), indirect draws, LOD chains | 6 |
| World Partition | Streamed terrain and props in tiles with LOD and impostors, for maps a kilometre across | 7 |
| Virtual shadow maps | Cascaded shadow maps (3-4 cascades, PCSS), plus a point-shadow atlas for the nearest lights | 3 |
| Many dynamic lights | Clustered (froxel) forward+: thousands of lights (every spell, projectile and torch) | 3 |
| Materials | PBR (GGX, metal/rough), procedural triplanar detail and normals, image-based light from a runtime sky cubemap | 2 |
| Post | HDR (rgba16float), physically based bloom, auto exposure, AgX/ACES, colour grading, vignette, depth of field (aim down the wand) | 2 |
| TSR | TAA with motion vectors and jitter, then temporal upscaling (render at 0.5-0.8, output native) | 4 |
| Screen-space | GTAO, SSR (water, marble, glass), contact shadows | 4 |
| Volumetrics | Froxel volumetric fog with light shafts; a volumetric storm wall (the shrinking zone) | 5 |
| Niagara | GPU particles: compute simulation, indirect draw, soft particles, trails, ribbons, mesh particles (spell VFX) | 5 |
| Foliage | Instanced grass and plants generated on the GPU, wind, interaction | 6 |
| Water | Reflective and refractive water with foam and caustics | 4 |

**Hard limits of WebGPU** (design around them, never fight them):
- No 64-bit atomics in core, so Nanite's software rasteriser cannot be
  ported. Do cluster culling and indirect draws instead.
- No bindless resources and no core multi-draw-indirect. Default
  `maxStorageBuffersPerShaderStage` is 8 and `maxBindGroups` is 4.
- Optional features vary by browser: `timestamp-query`, `float32-filterable`,
  `rg11b10ufloat-renderable`, `shader-f16`, subgroups,
  `indirect-first-instance`. Detect each one and keep a path without it.
- Support (2026):
  - Chrome and Edge 113+: Windows, macOS, ChromeOS; Android 121+ on many
    GPUs; Linux only on recent Intel and NVIDIA.
  - Safari 26 on macOS, iOS and iPadOS.
  - Firefox on Windows and Apple Silicon Macs only.
  - **A fallback is mandatory** (§2.4).

**Success:**
- The showcase page, on Ultra, looks like a UE5 scene: volumetric light,
  thousands of lights, soft shadows, GI, a kilometre of streamed terrain,
  heavy spell VFX.
- A mid-range phone holds 30+ fps on Medium.
- Luciphon runs on the engine with its gameplay unchanged.
- The netcode can carry a 60-player first-person match (§9).

---

## 1. What exists today (read these first)

**The page:** `crates/luciphon-web/src/`

| File | What it does |
|---|---|
| `lib.rs` | The frame loop: `frame` → net → chunk builds (2 a frame, nearest first) → `hands` (input, 30 Hz Inputs, prediction) → `draw` (builds a `scene::Frame`) → `overlay` (the HUD) → `present`. Also `camera()`: first person, the title orbit, the Underlight. |
| `scene/mod.rs` | `Scene`, `Camera`, `Frame`, `Hand`, `wand()`, `flame()`, `rarity()`. Draw order: sky → opaque chunks → things → transparent (ghosts, build ghost, node rings, beams) → additive sparks → hand (depth cleared). |
| `scene/chunk.rs` | One mesh per 32x32-tile chunk: ground (claim hue tint, uneven natural ground), cliffs to the void, rocky underside with stalactites, every object, and its static lights. |
| `scene/things.rs` | Procedural low-poly meshes: trees, rocks, crystals, moss, brambles, pillars, the Luciphon shrine and bell, hearth, walls, doors, lanterns, planters and crops; `lumen()`, `beast(kind)`, mote, glim, hand, cube, ring, rod; `light_of()`. |
| `scene/shapes.rs` | `Geo`: interleaved vertices (pos 3, normal 3, colour 3 + glow 1 = 10 floats), `tri`, `quad`, `column`, `block`, `blob`, `spike`. Flat shaded, wound outward (tested). |
| `scene/shaders.rs` | GLSL ES 3.0: the world shader (hemisphere ambient, moon, the 16 nearest point lights, glow, fog, the Underlight negative), sky (gradient and hashed stars), points (sparks). |
| `hud.rs`, `bag.rs`, `play.rs`, `first.rs` | The pixel HUD. Drawn into `kit::gl::Gl::hud` (a premultiplied `pixels::Canvas`) and uploaded as a texture each frame. |
| `fx.rs`, `state.rs`, `controls.rs`, `laws.rs` | CPU sparks and shake; mirror, prediction, interpolation and beams; input; `Feel` (FOV, eye height, fog, max DPR 2 desktop and 1.5 touch). |

**Elsewhere:**
- `crates/kit/src/gl.rs`: the WebGL2 screen (`Gl`, `Program`, `Mesh`, `m4`
  maths, column-major).
- `crates/kit/src/input.rs`: keys, fingers, mouse, pointer lock.
- `crates/render/src/sculpt.rs`: models sculpted from distance fields
  (blended and carved like clay), meshed smooth by surface nets with their
  creases darkened from the field; cloth woven as sheets. Wandfall's
  wizards are made with it (`crates/wandfall-look/src/rig/parts.rs`).

**Conventions:**
- **Space.** World tile `(x, y)` at height `h` is `(x, h, y)` in render space:
  x east, y up, z south. Headings are `u16` (65536 a turn; 0 east, 16384
  south). Camera forward is `(cos yaw cos pitch, sin pitch, sin yaw cos pitch)`.
- **Do not change gameplay, the server or the wire while building the
  renderer.** A pure page change never redeploys the server (CI compares
  build hashes).

**Numbers today:**
- The Luciphon wasm is 145 KB gzipped.
- CPU frame time is 1-3 ms (`?perf=1`).
- Headless SwiftShader renders the WebGL2 path correctly.

---

## 2. Architecture

### 2.1 Crates

```
crates/gpu       NEW, generic, no game knowledge. wgpu device/adapter/
                 surface (web + native); Caps (features, limits); render
                 targets; a small pass graph; buffer/texture/bind helpers; a
                 pipeline cache with async creation; GPU timestamps; the
                 pixel layer composite (a `pixels::Canvas`, premultiplied,
                 NEAREST, exactly as kit::gl does today).
crates/render    NEW, the engine, generic. A retained scene with handles:
                 meshes, materials, instances, lights, particle systems,
                 terrain, decals, the sky, the camera(s), a first-person
                 viewmodel layer, post settings, quality tiers. Games build
                 a scene; the engine draws it. Geometry builders (today's
                 `Geo`, `column`, `blob`, ...) move here so every game shares
                 them.
crates/net       LATER (Phase N): the netcode foundation shared by games
                 (§9). Nothing in it until the renderer's Phase 4 is done.
crates/showcase-web  NEW page `/showcase/`: a scene built only to show what
                 the engine can do and to test it (golden poses, perf
                 routes). No server needed.
crates/luciphon-web  Moves onto `render` in Phase 1. Its `scene/` (WebGL2)
                 stays, frozen, as the fallback tier.
```

- Every file stays under 1,000 lines (rule 6).
- WGSL lives in Rust strings (rule 1 allows GLSL in Rust strings; extend
  that wording to WGSL in `CLAUDE.md`).
- Tuning numbers live in `laws.rs` (rule 3): the engine's tiers and budgets
  go in `crates/render/src/laws.rs`; a game's look goes in its own `laws.rs`.

### 2.2 wgpu, and its versions (do this first)

Use **`wgpu`**:
- WebGPU on the web, Vulkan/Metal/DX12 natively.
- Native rendering gives fast iteration and golden-image tests in
  `cargo test`.
- Naga validates every WGSL string in unit tests.

**Version pins.**
- `Cargo.lock` pins `wasm-bindgen = 0.2.129`. Pick the newest `wgpu` whose
  web backend works with it, or bump wasm-bindgen workspace-wide.
- CI installs the wasm-bindgen CLI from `Cargo.lock` automatically, and
  `scripts/build-web.sh` needs the same version. Verify both after a bump.
- Async device creation needs `wasm-bindgen-futures`.

**Escape hatch.** If wgpu costs more than ~1.5 MB gzipped (measure in Phase
0), consider raw WebGPU through `web-sys` (`--cfg=web_sys_unstable_apis`).
Record the decision here.

### 2.3 The engine's API (a sketch; refine in Phase 1)

```rust
let mut r = render::Renderer::new(canvas_id, Tier::Auto).await?; // or GL fallback
let rock = r.mesh(&geo);                      // static geometry, LODs built for you
let stone = r.material(Material { base: rgb(150,150,160), rough: 0.8, ..Material::STONE });
let id = r.instance(rock, stone, Transform::at(x, h, z).yaw(a).scale(s));
r.light(Light::point(pos, colour, intensity, radius));      // static or moving
r.terrain(Heightfield { size, heights, materials });       // streamed, LOD'd
let sparks = r.particles(&SPARKS);                          // a GPU system
r.emit(sparks, Burst { at, count, speed, colour });
r.set(id, Transform::...);                                  // move things
r.remove(id);
r.draw(&Camera { eye, yaw, pitch, fov }, &Viewmodel { .. }, &hud_canvas, now);
```

- **Retained, not immediate.** Static worlds upload once. Dynamic things
  update transforms in a ring buffer; the engine never rebuilds the world
  per frame.
- **Luciphon's adapter** maps its `scene::Frame` into this API: chunks
  become terrain plus instances, things become instances, beams become
  light rods plus particles.

### 2.4 Tiers and the fallback

| Tier | Who | What |
|---|---|---|
| **GL** | No `navigator.gpu`, or the device fails | Today's WebGL2 renderer (Luciphon), or a minimal GL path for other games |
| **Low** | WebGPU, a weak adapter or a phone | HDR, tone map, bloom, clustered lights, 1 cascade, render scale 0.6 + TAAU |
| **Medium** | Phones that hold it, iGPUs | Low, plus 2 cascades, GTAO (half res), cheap froxel fog, GPU particles, near grass |
| **High** | Desktop GPUs | Medium, plus 3 cascades with PCSS, SSR, full froxel fog and god rays, grass to 30 m, 4 point shadows |
| **Ultra** | Strong desktops | High, plus DDGI, 4 cascades, 8 point shadows, native-res TAA, contact shadows |

- **Choosing a tier:**
  - Start from the adapter.
  - Measure the first 120 frames. Step down a tier if the 95th-percentile
    frame time exceeds budget; step up after 10 s comfortably under it.
  - `?q=gl|low|medium|high|ultra` forces a tier; `?gpu=0` forces GL.
  - Add a graphics setting to each game's menu; remember it in localStorage.

### 2.5 Determinism hooks for testing (Phase 0)

| Hook | What it does |
|---|---|
| `?cam=x,y,h,yaw,pitch` | Places a free debug camera. |
| `?t=ms` | Freezes render time. |
| `?hud=0` | Hides the HUD. |
| `?shot=1` | All of the above, plus a fixed resolution and render scale 1. Sets `document.title = "shot ready"` once frame N has fully rendered. |
| `?perf=1` | Extend the title line with tier, render scale, CPU and GPU ms, draws, lights after culling, triangles. On Luciphon, keep the existing tokens (`@x,y yaw ... beast d,a`): the test bots steer by them. |

---

## 3. Phases (renderer)

**Progress (October 2026, cloud session).** Luciphon is frozen (legacy), so
its adapter is dropped; Wandfall and the showcase are the engine's users.
Done, across phases, in `crates/render`:
- Phase 1: the retained scene (meshes, now indexed; statics; moving items;
  materials: plain, terrain, foliage, water, metal), the light grid, sky,
  air, the see-through and glowing passes, sparks, the viewmodel layer.
  Geometry: smooth spheres (icosahedral, bumped), lathed shapes, smoothing.
- Phase 2: HDR (rgba16float) with 4x MSAA, bloom (a 13-tap halving chain
  and tent upsampling), ACES, a vignette, sRGB; GGX specular with a sky
  reflection for ambient sheen; procedural surface detail (noise bumps)
  instead of textures; the terrain material (grass, dry grass, rock by
  slope, sand and wet sand by the sea). A grade after tone mapping
  (`Look::grade`: lift, gamma, gain per channel, saturation, an S-curve
  contrast) and a dither against banding. Cloth (wrapped light and a
  Charlie sheen, its colour lighter) and skin (light wrapped further in
  red) materials. `Look::glow` scales all that glows (emission, sparks,
  point lights), so a look exposed for a dark night keeps a spell its
  colour instead of burning it white. Screen-space reflections on the
  sea (`shaders::world` `mirror`, `Quality::ssr` steps: 16 high, 10
  medium, none low; `?ssr=0`): the picture so far is resolved into a
  copy before the see-through pass, and the sea marches its reflected
  ray (off waves calmed by two thirds) out to 120 m in growing steps,
  refines where it first passes behind what stands there, and mixes that
  over the sky's reflection, fading at the screen's edge and far off.
  What glows is drawn after the copy, so it is not mirrored yet. Not yet:
  auto exposure, DoF, IBL.
- Phase 5 (part): sparks with shapes (`Shape`: a glow, a licking flame,
  a puff of smoke laid over rather than added, a four-rayed glint), each
  stretched along its motion into a streak (`Spark::v`), flames and
  puffs let grow to a share of the screen, every spark fading right by
  the eye; an `Energy` material (noise flowing over the surface: fire,
  ragged at its edges, or plasma, bright at its rim). Soft sparks: in
  the pass after what is solid (depth only read), each fades as it
  nears what stands behind it, over half its size, so a puff meeting
  the ground does not cut a line (Medium and High). Not yet: GPU
  simulation, ribbons. Shafts of sunlight (`shafts.rs`):
  from the depth at a quarter of the screen, each pixel marches toward
  the sun on screen over the open sky near it, dimming as it goes; the
  finish adds them in the sun's colour before tone mapping (High and
  Medium; `?shafts=0` turns them off).
- Phase 3: the sun's cascaded shadows (3 x 2048 on High, PCF 3x3, snapped
  to texels). Not yet: static caster caching, point shadows, froxels.
- Phase 4 (part): ambient occlusion (`ao.rs`, `shaders/ao.rs`), HBAO
  from the depth at half res: the facing rebuilt from the depth, 6 ways
  out (4 on Medium, none on Low) a pixel, 4 steps each, turned by
  interleaved gradient noise; a 4x4 blur kept to its depth; then a
  depth-aware upsample multiplied into the picture once what is solid is
  drawn, before what is see-through, glows and the viewmodel. `?ao=0`
  turns it off; its numbers are in `laws.rs` (`AO_*`). The sea (waves,
  fresnel sky, shallows from the terrain's heights, glints, foam). Phase 5 (part): the sky (gradient, sun glow,
  disc, drifting clouds) and height fog that takes the sky's colour.
- Phase 6 (part): GPU grass (100k+ blades about the eye, wind, rooted on
  the heights texture, none on sand, rock or steep ground).
- Tiers: `Quality::{HIGH, MEDIUM, LOW}`; `?q=` forces one; software
  adapters get Low, touch screens Medium.

Every phase:
1. Keep all gates green (`CLAUDE.md` § Commands).
2. Run the parallel sweep (§4) before pushing.
3. Post before/after screenshots and numbers here.
4. Push to a `claude/` branch (it deploys).
5. Ship each phase behind its tier.

### Phase 0: spike and plumbing (1-2 days)
- [x] `crates/gpu`: device on the web, `Caps`, a surface sized like
      `kit::gl::Gl` (`kit::gl::measure`, DPR caps), and the pixel-layer
      composite (`layer.rs`: one fullscreen triangle, `textureLoad`, so the
      layer stays sharp). The adapter is asked for before the canvas is
      touched, so a failure leaves it free for WebGL2. Optional features
      (timestamps, f16, filterable f32, RG11B10 targets) come when offered.
      *Native device: not yet* (off the web the crate builds its shaders
      only); it comes with golden images in Phase 1.
- [x] `crates/showcase-web` + `web/showcase/index.html` + a `page` line in
      `scripts/build-web.sh` + its pkg path in `web/vercel.json`. It clears,
      draws a test triangle, and shows the HUD layer.
- [x] §2.5 hooks; renderer choice and the GL fallback at start. Done:
      `?t=ms`, `?hud=0`, `?shot=1` ("shot ready" after 3 frames), `?perf=1`
      (`showcase webgpu 45fps first 74ms`), `?gpu=0`. Later: `?cam=` (no
      camera yet) and the fixed resolution for `?shot=1`.
- [ ] Measure: wasm size (gzipped), time to first frame, device creation
      time on Chrome, Safari and Firefox. **Measured (October 2026):** the
      showcase wasm is 342,182 bytes, 106,415 gzipped (wgpu's WebGPU
      backend, the GL fallback and the HUD; Luciphon's page is 154,671
      gzipped). Headless Chromium 1194 on SwiftShader: WebGPU found
      (`--enable-unsafe-webgpu --use-webgpu-adapter=swiftshader`), first
      frame 52-130 ms from page open (device creation included), 36-48 fps
      in software; without an adapter it falls back to WebGL2 (first frame
      63-113 ms). Live at `/showcase/` (Vercel): first frame 776 ms on
      WebGPU, 826 ms on WebGL2, download included (through a proxy).
      *Left: real GPUs, Safari and Firefox.*
- [ ] Native tests: an empty frame offscreen (skip if no adapter) *(with
      the native device)*, and Naga validation of every WGSL string *(done:
      `crates/gpu/tests/wgsl.rs`, `crates/showcase-web/tests/wgsl.rs`; every
      new WGSL string gets one)*.

### Phase 1: the engine core, and Luciphon's look on it (3-5 days)
- [ ] `crates/render`:
  - meshes (moving `Geo` and its builders here), instances, materials (flat
    colour + glow for now), static and dynamic point lights;
  - sky, fog, depth, the transparent and additive passes, the viewmodel
    layer (separate FOV, its own depth clear), the camera.
- [ ] Luciphon renders through `render` (an adapter from `scene::Frame`),
      looking as it does today.
- [ ] The showcase scene, v1:
  - a shrine with a glowing bell, a forest, rocks, crystals and water;
  - a few hundred lights;
  - a wizard's wand in view, casting.
- [ ] Golden images (§4.2): GL vs WebGPU differ by under 2% of pixels (stars
      excepted). Perf no worse than GL. WebGPU becomes the default where
      available.

### Phase 2: HDR, PBR, post (3-5 days)
- [ ] An rgba16float target, with physically based bloom (dual filter,
      energy-conserving).
- [ ] Auto exposure: a compute histogram, then an eased average.
- [ ] AgX/ACES tone map, then a grading LUT. Vignette and slight chromatic
      aberration on High and Ultra.
- [ ] Depth of field (for aiming down the wand later).
- [ ] PBR (GGX, metal/rough, energy compensation):
  - Material library: stone, marble, wood, foliage, crystal, glass, water,
    cloth, metal, emissive.
  - Procedural triplanar detail and normal maps, generated by compute at
    load. **No downloaded textures** unless the owner allows them.
- [ ] IBL: render the sky into a cubemap, prefilter it, add SH ambient.
- [ ] Emissives push into bloom.

### Phase 3: lights and shadows (4-6 days)
- [ ] Clustered forward+ (froxels 16x9x24; compute assignment; a light
      SSBO). Target 2,000 lights in under 1.5 ms on High.
- [ ] Spells, projectiles, torches and hits as light sources, with
      short-lived lights pooled.
- [ ] Sun/moon cascaded shadow maps:
  - 3-4 cascades, stable snapping, PCF; PCSS on High and Ultra.
  - **Cache static casters**; re-render only dynamic ones.
- [ ] A point-shadow atlas for the N most important lights, with static
      casters cached.
- [ ] Contact shadows on Ultra.

### Phase 4: temporal and screen-space (4-6 days)
- [ ] Motion vectors (camera, plus per instance from previous transforms).
- [ ] TAA (jitter, neighbourhood clamp, disocclusion rejection), then TAAU
      (render scale per tier). This is what makes phones possible.
- [x] AO at half res with a bilateral upsample (HBAO, not yet GTAO).
- [ ] SSR (hierarchical depth), falling back to IBL.
- [ ] Water:
  - normal-mapped ripples;
  - SSR reflection and refraction;
  - shore foam and caustics.

### Phase 5: atmosphere and VFX (4-6 days)
- [ ] Froxel volumetric fog:
  - height and distance density, in-scattering from clustered lights and
    the sun or moon;
  - shadowed shafts (god rays) and temporal reprojection.
- [ ] **A storm wall**: a volumetric, animated boundary with a moving
      radius. It is the battle royale's shrinking zone, and it must look
      majestic and read clearly from inside and outside.
- [ ] GPU particles:
  - compute simulation, append/consume buffers, indirect draw;
  - soft particles, curl noise, ribbons and trails, mesh particles;
  - an emitter API in `render`.
  - VFX library v1 (for wand spells): bolt, beam, nova (AoE ring), shield
    bubble, heal motes, blink trail, root/snare chains, impact bursts,
    scorch decals.
- [ ] Decals (projected boxes): scorch marks, spell circles on the ground,
      the build ghost.

### Phase 6: GPU-driven geometry and foliage (5-8 days)
- [ ] Instancing everywhere: one mesh per kind with 2-3 LODs (a simple Rust
      quadric or vertex-clustering simplifier) and per-instance transform,
      tint and material.
- [ ] Culling statics. Tried (October 2026): statics in chunks of the
      ground (48 or 80 m) culled against the view's and each cascade's
      side planes cut triangles only ~20% (1220k to 950k on the range) but
      split the runs, so draws went from 410 to 900-1260: reverted. The
      way: GPU-driven, instance data in a storage buffer read through a
      per-frame list of visible indices (a few KB a frame), so culling
      adds no draws.
- [x] The sun's cascades culled (`render/src/cull.rs`): each cascade
      lays out its own instances of what could cast into its box (its
      bounding sphere inside the box seen from the sun, not beyond its far
      side, with slack), again only when the box has moved past the slack
      or the sun has turned, so runs stay one a mesh and draws do not grow
      (400 to 360). Past the nearest cascade statics cast at their far
      mesh, and what moves always does (a wizard's shadow from its 7k
      model, not its 42k one). The ground is cut in 4 by 4 pieces so the
      cascades take only the near ones. On the range, shadow triangles
      3,395k to 2,250k; `?perf=1` shows them a cascade at a time.
- [x] The view culled the same way: statics within a cone 20 degrees
      wider than the eye's (or within 6 m), laid again in the frame the
      view turns past that or the eye moves 6 m, so nothing pops; one run
      a mesh still. On the range 1,238k triangles to 910k, instances 1,840
      to 1,148, draws 360 to 346.
- [ ] Compute culling: frustum plus Hi-Z occlusion from last frame's depth
      pyramid, writing indirect args. A path without
      `indirect-first-instance`.
- [ ] Grass and plants:
  - generated on the GPU from terrain materials;
  - wind, and bending around characters;
  - distance fade.
- [ ] Vertex animation: tree and cloth sway; procedural creature and
      character motion (legs, bob, lean, float).
- [ ] Skinned meshes (bones, GPU skinning) for future characters and wands,
      with a small animation blender. Meshes are still built in code or from
      a compact in-repo format; no glTF loader unless the owner wants
      artists' assets.

### Phase 7: big worlds (5-8 days)
- [ ] Heightfield terrain:
  - clipmap or chunked LOD, crack-free, materials blended by splat weights;
  - cliffs as extra meshes;
  - 1-2 km across at 0.5-1 m per sample.
- [ ] Streaming: tiles of terrain and props loaded by distance, impostors
      (octahedral billboards baked at load) for far trees and rocks, and a
      memory budget.
- [ ] A far horizon: distant terrain silhouettes and the atmosphere.
- [ ] The showcase becomes a 1 km valley with a ruin, a lake, a forest and a
      storm wall closing in: the battle royale's scale.

### Phase 8: global illumination (5-10 days, High and Ultra)
- [ ] DDGI-lite:
  - a probe grid; budgeted updates per frame by ray-marching a coarse voxel
    or SDF of the scene, built on load and updated when static geometry
    changes;
  - octahedral irradiance and visibility;
  - sampled in the lighting pass.
- [ ] Emissive surfaces feed GI. SSGI on Ultra. Reflections sample probes
      beyond SSR.

### Phase 9: tune, phones, polish (ongoing)
- [ ] Budgets (§5) met on the test matrix (§4.4); automatic tier stepping
      proven.
- [ ] Pipelines created async at load (a progress line), never mid-play.
- [ ] Device loss: rebuild from the retained scene without crashing.
- [ ] GPU memory under 300 MB on Medium and 150 MB on Low.

---

## 4. Testing: massively parallel, with Chrome and subagents

The orchestrator runs a **sweep** after every meaningful change. Subagents
run in parallel, each in its own Chrome window or tab (separate contexts are
separate players). Each writes:

```
docs/engine-runs/<date>-<phase>/<role>.md
```

The report holds numbers, failures and repro URLs. Commit the reports; keep
screenshots local unless one shows a bug or a milestone (under 200 KB each).

### 4.1 Local setup
- `bash scripts/build-web.sh && cargo run -p secretspace-server --release --
  --static dist` serves everything on `:8787`. The showcase needs no server
  beyond the static files.
- **Real WebGPU:** use Chrome with Claude (headed).
- **Headless Playwright** (preinstalled Chromium):
  - WebGL2 works with `--use-gl=angle --use-angle=swiftshader
    --enable-unsafe-swiftshader --ignore-gpu-blocklist`.
  - For WebGPU, try `--enable-unsafe-webgpu` (with Vulkan/SwiftShader
    flags). Check `await navigator.gpu?.requestAdapter()`.
  - If headless gets no adapter, use headless for the GL path and goldens,
    and headed Chrome for WebGPU.
- **Pointer lock under Playwright:** mouse moves must be **cumulative**
  (`mx += d; mouse.move(mx, y)`); moving out and back nets zero `movementX`.
- **Luciphon bots** read `document.title` under `?perf=1` (`@x,y yaw Y beast
  dist,angle`) to steer: walk out to the Rim, hunt the nearest beast.
- **Phones:** emulate twin thumbs with CDP `Input.dispatchTouchEvent`.

### 4.2 Golden poses (`?shot=1`)

Each pose runs on GL where it applies, and on every WebGPU tier.

**Showcase:**
1. The bell shrine, close.
2. The forest at dusk.
3. The lake (SSR, refraction).
4. A crystal cave (emissive, GI).
5. A storm wall from inside.
6. The same, from outside.
7. A spell storm: 50 spells, 1,000 lights.
8. A 1 km vista (streaming, impostors).
9. Straight up (sky).
10. Phone portrait 390x844, DPR 3, Medium.

**Luciphon:**
1. The Sanctum.
2. The Dim with beasts.
3. The Rim edge, looking down.
4. A built hearth.
5. The Underlight.

A diff tool in the harness (Node `pixelmatch` is fine: rule 1 governs shipped
pages, not test harnesses) reports the changed pixels against the accepted
images.

### 4.3 Subagent roles (all in parallel)

| Role | Does | Pass |
|---|---|---|
| **Golden** | Every §4.2 pose on every tier; diff against accepted | No unexplained diff over 2%; new effects reviewed |
| **Perf** | 60 s fixed camera routes per tier; CPU and GPU ms p50/p95/p99, draws, lights, triangles | Within §5 |
| **Players x4-8** | Luciphon: join, fight in the Dim, craft, fall, respawn | No page errors; no prediction snaps; steady frames |
| **Phone** | Touch play; Medium and Low; text fits | 30 fps on a real phone when available |
| **Fallback** | `?gpu=0`; a browser without WebGPU | Identical to today |
| **Soak** | 30 min: tier switches, streaming in and out, tab hidden and shown | GPU memory growth under 10%; device-loss recovery within 1 s |
| **Stillness** | Restart the server mid-play | The picture holds, then resumes |
| **Browsers** | Chrome, Edge, Safari 26 (macOS/iOS), Firefox (Windows/macOS) | Works, or steps down cleanly; quirks recorded |

### 4.4 Device matrix (as available)
- A discrete-GPU desktop.
- A laptop iGPU.
- An Apple Silicon Mac (Safari and Chrome).
- An Android phone.
- An iPhone (Safari 26).
- Emulate what you lack, and note the gap.

---

## 5. Budgets

| | Ultra | High | Medium | Low | GL |
|---|---|---|---|---|---|
| GPU ms (p95) | 12 | 8 | 14 | 16 | as today |
| Render scale | 1.0 | 0.8-1.0 | 0.66 | 0.5 | DPR cap |
| Lights after culling | 4,000 | 2,000 | 512 | 256 | 16 |
| Shadow cascades | 4 | 3 | 2 | 1 | 0 |

- **Targets per machine:**
  - Ultra and High: a mid-range desktop GPU (RTX 3060 / M1 class) at 1440p.
  - Medium: a mid-range phone at DPR 1.5.
  - Low: an old phone at 30 fps.
- **Page size:** a game page at most **1.5 MB gzipped** (Luciphon is 145 KB
  today). No texture downloads.
- **Load:** a title within 3 s; pipelines ready within 6 s; never a blank
  page.

---

## 6. Risks

| Risk | Mitigation |
|---|---|
| wgpu ↔ wasm-bindgen versions | Settle in Phase 0; bump workspace-wide if needed |
| wasm size | Measure every phase; compile-time feature gates; the raw web-sys hatch |
| Safari/Firefox quirks | Feature detection; the Browsers role every phase; step down, never crash |
| Shader compile hitches | Every pipeline async at load |
| Phone heat and throttling | Tier stepping on p95; TAAU first; GI and volumetrics off on phones |
| Breaking Luciphon | The adapter only reads `scene::Frame`; core, server and wire untouched |
| Scope creep into game design | The game (`docs/plunder.md`) waits; build only what the showcase and Luciphon need, plus §9 when its turn comes |

---

## 7. Working rules
- All of `CLAUDE.md` applies:
  - update rule 1's wording to allow WGSL in Rust strings;
  - files under 1,000 lines;
  - numbers in `laws.rs`;
  - wasm clippy green; fmt; caps.
- Add `-p` entries for the new web crates to rule 7's clippy command and to
  CI.
- Small commits, one effect at a time, each behind its tier. Push to a
  `claude/` branch to deploy, then verify live (`/health`, the showcase,
  `/luciphon/?perf=1`).
- After each phase, update `CLAUDE.md`'s map (under 8,000 characters) and
  this file's boxes.
- **Ask the owner** before:
  - raising the page-size budget;
  - adding downloaded assets;
  - choosing a host or transport for §9;
  - changing gameplay.

---

## 8. Open questions for the owner (defaults in brackets)
1. Art direction: stylised with UE5 lighting, or realism with downloaded
   textures and models? [Stylised, procedural, no downloads, for now.]
2. Minimum device: is a 2020-era phone a target? [Yes, on Low.]
3. Time of day in the showcase: a full day cycle (sun, sky, stars)? [Yes:
   it exercises shadows, GI and exposure.]

---

## 9. Phase N: the netcode foundation (after the renderer's Phase 4)

"Ultra high performance on network" for a 60-player first-person wand
shooter. Today: WebSocket (TCP), 30 Hz, deltas per entity, interest by
distance, client prediction (Luciphon), and target rewind by RTT/2 + 66 ms
(`hits.rs::rewind`). Build `crates/net` from what works:

- [ ] **A tick and snapshot core.**
  - 60 Hz server simulation.
  - Per-client snapshot deltas against the last acknowledged snapshot.
  - Quantized fields and bit-packing.
  - A bandwidth budget per client (start at 64 kbit/s down at 60 players)
    with priority accumulation (what is near, aimed at or fast goes first).
- [ ] **Prediction and reconciliation**, generic over a game's `step`
      (Luciphon's `predict.rs` is the model), and interpolation buffers sized
      by measured jitter.
- [ ] **Lag compensation** for hitscan and projectiles: a history ring of
      hitboxes, the server rewinding to the shooter's view time (capped at
      ~200 ms), and favour-the-shooter within limits.
- [ ] **Transport.** WebSocket stays the baseline. Add an unreliable channel
      for state where hosting allows:
  - **WebTransport datagrams** (HTTP/3 over QUIC/UDP) or WebRTC data
    channels. **Railway's public networking may not carry UDP; verify,
    and ask the owner before changing host** (Fly.io and others do).
  - Keep the server std-only unless the owner agrees to a QUIC dependency.
- [ ] **Load test:**
  - Rust bots that speak the protocol (native, many per process);
  - 60 per match × N matches on one server;
  - measure CPU per tick, bandwidth per client, and p99 latency.
- [ ] **Matches** (for the battle royale): a queue room, match rooms created
      and destroyed by the server, and spectating. `engine::room` is per-room
      threads today, which is fine for dozens of matches per process.
