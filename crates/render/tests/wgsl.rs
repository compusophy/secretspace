//! The engine's WGSL parses and validates (naga, as wgpu compiles it),
//! has the entry points the pipelines name, and binds what the layouts
//! hold (`render::slots`), for every tier's variant of each module.

use render::slots::{self, Seen, Slot};

fn module(name: &str, src: &str) -> naga::Module {
    let module = naga::front::wgsl::parse_str(src)
        .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(src)));
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::default(),
    )
    .validate(&module)
    .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(src)));
    module
}

fn valid(name: &str, src: &str, entries: &[&str]) -> naga::Module {
    let module = module(name, src);
    for entry in entries {
        assert!(
            module.entry_points.iter().any(|e| e.name == *entry),
            "{name}: {entry}"
        );
    }
    module
}

/// What the WGSL binds at each (group, binding), as a layout's slot.
fn bound(module: &naga::Module) -> Vec<(u32, u32, Slot)> {
    use naga::{AddressSpace, ImageClass, ImageDimension, ScalarKind, TypeInner};
    module
        .global_variables
        .iter()
        .filter_map(|(_, v)| {
            let at = v.binding.as_ref()?;
            let slot = match (&v.space, &module.types[v.ty].inner) {
                (AddressSpace::Uniform, _) => Slot::Uniform,
                (AddressSpace::Storage { .. }, _) => Slot::Storage,
                (_, TypeInner::Sampler { comparison }) => Slot::Sampler {
                    compare: *comparison,
                },
                (
                    _,
                    TypeInner::Image {
                        dim: ImageDimension::D2,
                        arrayed,
                        class,
                    },
                ) => match class {
                    ImageClass::Depth { multi } => Slot::Depth {
                        array: *arrayed,
                        msaa: *multi,
                    },
                    ImageClass::Sampled {
                        kind: ScalarKind::Float,
                        multi: false,
                    } if !arrayed => Slot::Float { filterable: true },
                    other => panic!("{:?}: {other:?}", v.name),
                },
                other => panic!("{:?}: {other:?}", v.name),
            };
            Some((at.group, at.binding, slot))
        })
        .collect()
}

/// Every binding the WGSL has is in its group's table, of its kind (a
/// float texture may be filterable or not: the sampler decides).
fn binds(name: &str, module: &naga::Module, groups: &[&[(Slot, Seen)]]) {
    for (group, binding, slot) in bound(module) {
        let table = groups
            .get(group as usize)
            .unwrap_or_else(|| panic!("{name}: no group {group}"));
        let (want, _) = table
            .get(binding as usize)
            .unwrap_or_else(|| panic!("{name}: no binding {group}.{binding}"));
        let same = match (slot, *want) {
            (Slot::Float { .. }, Slot::Float { .. }) => true,
            (a, b) => a == b,
        };
        assert!(
            same,
            "{name}: {group}.{binding} is {slot:?}, the layout {want:?}"
        );
    }
}

#[test]
fn the_scene_shaders_are_valid_and_bind_what_the_layouts_hold() {
    for msaa in [false, true] {
        for ssr in [0, 16] {
            let name = format!("scene (msaa {msaa}, ssr {ssr})");
            let m = valid(
                &name,
                &render::shaders::scene(msaa, ssr),
                &[
                    "world_vs",
                    "world_fs",
                    "faint_fs",
                    "glow_soft_fs",
                    "sky_vs",
                    "sky_fs",
                    "spark_vs",
                    "spark_fs",
                    "spark_soft_fs",
                    "grass_vs",
                    "grass_fs",
                    "decal_vs",
                    "decal_fs",
                ],
            );
            binds(&name, &m, &[&slots::SCENE, &slots::soft(msaa)]);
        }
    }
}

#[test]
fn both_tone_maps_are_valid() {
    let post = render::shaders::post();
    for map in ["aces", "neutral"] {
        let src = post.replace(
            &format!("{}(c);", render::laws::TONE_MAP),
            &format!("{map}(c);"),
        );
        valid(map, &src, &["finish_fs"]);
    }
}

#[test]
fn the_shadow_and_post_shaders_are_valid_and_bind_what_the_layouts_hold() {
    let m = valid("shadow", &render::shaders::shadow(), &["shadow_vs"]);
    binds("shadow", &m, &[&slots::CASTER]);
    let m = valid(
        "post",
        &render::shaders::post(),
        &["post_vs", "down_fs", "up_fs", "finish_fs"],
    );
    binds("post", &m, &[&slots::FINISH]);
    for (k, (slot, _)) in slots::POST.iter().enumerate() {
        assert_eq!(
            *slot,
            slots::FINISH[k].0,
            "bloom's group is the finish's first"
        );
    }
    for msaa in [false, true] {
        let m = valid(
            "ao",
            &render::shaders::ao::ao(msaa),
            &["ao_vs", "ao_fs", "blur_fs", "apply_fs"],
        );
        binds("ao", &m, &[&slots::ao(msaa)]);
        let m = valid(
            "shafts",
            &render::shaders::shafts::shafts(msaa),
            &["shafts_vs", "shafts_fs"],
        );
        binds("shafts", &m, &[&slots::shafts(msaa)]);
    }
}

#[test]
fn globals_are_the_fields_the_renderer_writes() {
    let m = module("scene", &render::shaders::scene(true, 16));
    let (_, ty) = m
        .types
        .iter()
        .find(|(_, t)| t.name.as_deref() == Some("Globals"))
        .expect("Globals");
    let naga::TypeInner::Struct { members, span } = &ty.inner else {
        panic!("Globals is a struct");
    };
    let names: Vec<&str> = members.iter().filter_map(|m| m.name.as_deref()).collect();
    assert_eq!(names, render::shaders::FIELDS);
    assert_eq!(*span as u64, render::shaders::GLOBALS);
}
