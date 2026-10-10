//! The skeleton, frame by frame: feet planted and on the ground, the wand
//! along its aim, nothing jumping between frames.

use super::*;

fn pose() -> Pose {
    Pose {
        spell: None,
        flash: 0.0,
        arm: 0.0,
        aim: 0.0,
        tip: ([1.0; 3], 0.0),
        glide: false,
        t: 0.0,
    }
}

fn sole(f: &Frames, side: usize) -> V3 {
    point(&f.boot[side], [0.0, -ANKLE, 0.0])
}

fn run(a: &mut Anim, from: V3, v: (f32, f32), yaw: f32, frames: usize) -> V3 {
    let mut p = from;
    for _ in 0..frames {
        p = [p[0] + v.0 / 60.0, 0.0, p[2] + v.1 / 60.0];
        a.step(p, (yaw, 0.0), (true, false, false), 1.0 / 60.0);
    }
    p
}

#[test]
fn it_stands_on_its_feet_and_falls_on_its_back() {
    let f = wizard([0.0; 3], 0.0, &Anim::default(), &pose(), 1);
    let head = point(&f.head, [0.0, 0.14, 0.0]);
    assert!(head[1] > 1.4, "the head is up: {head:?}");
    for side in 0..2 {
        assert!(
            sole(&f, side)[1].abs() < 0.02,
            "on the ground: {:?}",
            sole(&f, side)
        );
    }
    let f = fallen([0.0; 3], 0.0, &pose(), 1, 1.0, 0.0);
    let head = point(&f.head, [0.0, 0.14, 0.0]);
    assert!(head[1] < 0.5 && head[0] < -1.0, "fallen backward: {head:?}");
}

#[test]
fn a_knockout_buckles_first_then_goes_over_its_own_way() {
    // Its knees give first: the hips drop, the feet stay on the ground.
    let stood = point(
        &fallen([0.0; 3], 0.0, &pose(), 1, 0.0, 0.0).pelvis,
        [0.0; 3],
    );
    let f = fallen([0.0; 3], 0.0, &pose(), 1, 0.12, 0.0);
    let hips = point(&f.pelvis, [0.0; 3]);
    assert!(
        stood[1] - hips[1] > 0.25,
        "buckled: {hips:?} from {stood:?}"
    );
    for side in 0..2 {
        assert!(
            sole(&f, side)[1].abs() < 0.05,
            "feet down: {:?}",
            sole(&f, side)
        );
    }
    // Then over, turned the way it twists, and a bounce off the ground.
    let head = |twist: f32, t: f32| {
        let f = fallen([0.0; 3], 0.0, &pose(), 1, t, twist);
        point(&f.head, [0.0, 0.14, 0.0])
    };
    let (left, right) = (head(-0.8, 1.0), head(0.8, 1.0));
    assert!(left[1] < 0.5 && right[1] < 0.5, "{left:?} {right:?}");
    assert!(left[2] > 0.5 && right[2] < -0.5, "{left:?} {right:?}");
    let lowest = (50..70)
        .map(|k| head(0.0, k as f32 / 100.0)[1])
        .fold(9.0, f32::min);
    assert!(head(0.0, 0.62)[1] > lowest + 0.02, "it bounces");
}

#[test]
fn a_cast_points_the_wand_along_its_aim() {
    let mut p = pose();
    p.arm = 1.0;
    let f = wizard([0.0; 3], 0.0, &Anim::default(), &p, 1);
    let shoulder = point(&f.upper[1], [0.0; 3]);
    assert!(f.tip()[0] - shoulder[0] > 0.8, "ahead: {:?}", f.tip());
    // Along the aim, however the chest leans: standing, at a run, at a
    // sprint, crouched and crouch-walking.
    for aim in [0.0f32, 0.4, -0.3] {
        p.aim = aim;
        for (v, crouch) in [
            (0.0, false),
            (7.0, false),
            (12.0, false),
            (0.0, true),
            (4.0, true),
        ] {
            let mut a = Anim::default();
            let mut at = [0.0f32; 3];
            for _ in 0..90 {
                at[0] += v / 60.0;
                a.step(at, (0.0, aim), (true, crouch, false), 1.0 / 60.0);
            }
            let f = wizard(at, 0.0, &a, &p, 1);
            let d = dir(&f.fore[1], [0.0, -1.0, 0.0]);
            let pitch = d[1].atan2(d[0]);
            assert!(
                (pitch - aim).abs() < 0.05,
                "aim {aim} at {v} m/s crouched {crouch}: the wand at {pitch}"
            );
        }
    }
}

/// A run along x at `v` m/s with a jump `secs` long from `from`: each
/// frame's soles, and whether it was on the ground.
fn jump(a: &mut Anim, from: V3, v: f32, secs: f32) -> (V3, Vec<([V3; 2], bool)>) {
    let mut p = from;
    let mut seen = Vec::new();
    let up = secs * 12.5;
    for k in 0..(secs * 60.0) as usize + 12 {
        let t = (k + 1) as f32 / 60.0;
        let y = (up * t - 12.5 * t * t).max(0.0);
        let ground = y <= 0.0;
        p = [p[0] + v / 60.0, y, p[2]];
        a.step(p, (0.0, 0.0), (ground, false, false), 1.0 / 60.0);
        let f = wizard(p, 0.0, a, &pose(), 1);
        seen.push(([sole(&f, 0), sole(&f, 1)], ground));
    }
    (p, seen)
}

#[test]
fn landing_its_feet_meet_the_ground_at_once() {
    // Running at 6 m/s, a jump of 0.6 s: falling, the legs reach down; a
    // few frames after it lands, a foot is planted.
    let mut a = Anim::default();
    let at = run(&mut a, [0.0; 3], (6.0, 0.0), 0.0, 60);
    let (_, seen) = jump(&mut a, at, 6.0, 0.6);
    let land = seen.iter().position(|s| s.1).unwrap();
    let low = |s: &([V3; 2], bool)| s.0[0][1].min(s.0[1][1]);
    let last_up = low(&seen[land - 1]);
    assert!(last_up < 0.2, "falling, a foot reaches down: {last_up}");
    let planted = seen[land..land + 3].iter().map(low).fold(9.0, f32::min);
    assert!(planted < 0.03, "a foot down within 3 frames: {planted}");
    // Hopping, a frame or two on the ground each time: each touches down.
    let mut a = Anim::default();
    let mut at = run(&mut a, [0.0; 3], (8.0, 0.0), 0.0, 60);
    for _ in 0..4 {
        let (p, seen) = jump(&mut a, at, 8.0, 0.5);
        at = p;
        let down = seen.iter().filter(|s| s.1).map(low).fold(9.0, f32::min);
        assert!(down < 0.04, "a hop touches down: {down}");
    }
}

#[test]
fn rising_it_leaps_and_a_jump_in_the_air_tucks_and_kicks() {
    let mut a = Anim::default();
    let mut p = run(&mut a, [0.0; 3], (7.0, 0.0), 0.0, 60);
    // Rising: one knee up ahead, the other leg trailing.
    for k in 0..9 {
        let t = (k + 1) as f32 / 60.0;
        p = [p[0] + 7.0 / 60.0, 7.0 * t - 12.5 * t * t, p[2]];
        a.step(p, (0.0, 0.0), (false, false, false), 1.0 / 60.0);
    }
    let f = wizard(p, 0.0, &a, &pose(), 1);
    let (l, r) = (sole(&f, 0), sole(&f, 1));
    let (front, back) = if l[0] > r[0] { (l, r) } else { (r, l) };
    assert!(front[0] - back[0] > 0.35, "a leap: {front:?} {back:?}");
    assert!(a.flip == 0.0 && a.wall.is_none());
    // Falling, then a jump made in the air: it tucks.
    let mut vy = -2.0;
    for _ in 0..20 {
        vy -= 25.0 / 60.0;
        p = [p[0] + 7.0 / 60.0, p[1] + vy / 60.0, p[2]];
        a.step(p, (0.0, 0.0), (false, false, false), 1.0 / 60.0);
    }
    for _ in 0..4 {
        p = [p[0] + 7.0 / 60.0, p[1] + 7.0 / 60.0, p[2]];
        a.step(p, (0.0, 0.0), (false, false, false), 1.0 / 60.0);
    }
    assert!(a.flip > 0.5 && a.wall.is_none(), "{a:?}");
    let f = wizard(p, 0.0, &a, &pose(), 1);
    for side in 0..2 {
        assert!(
            sole(&f, side)[1] - p[1] > 0.3,
            "knees up: {:?}",
            sole(&f, side)
        );
    }
    // Running at a wall (+x), a kick back off it (-x) and up: the leg on
    // its side stretches back toward it.
    let mut a = Anim::default();
    let mut p = run(&mut a, [0.0; 3], (0.0, 6.0), 0.0, 60);
    for _ in 0..12 {
        p = [p[0], p[1] + 2.0 / 60.0, p[2] + 6.0 / 60.0];
        a.step(p, (0.0, 0.0), (false, false, false), 1.0 / 60.0);
    }
    for _ in 0..4 {
        p = [p[0] - 7.0 / 60.0, p[1] + 8.5 / 60.0, p[2]];
        a.step(p, (0.0, 0.0), (false, false, false), 1.0 / 60.0);
    }
    let wall = a.wall.expect("a kick off a wall");
    assert!(
        (wall - FRAC_PI_2).abs() < 0.4,
        "the wall to its right: {wall}"
    );
    let f = wizard(p, 0.0, &a, &pose(), 1);
    let kick = sole(&f, 1);
    assert!(
        kick[2] - p[2] > 0.3,
        "the right leg out toward it: {kick:?} from {p:?}"
    );
}

#[test]
fn on_its_broom_it_bobs_with_it_and_banks_into_a_turn() {
    let mut a = Anim::default();
    a.seat.x = 1.0;
    let mut p = pose();
    let gap = |a: &Anim, p: &Pose| {
        let f = wizard([0.0; 3], 0.0, a, p, 1);
        point(&f.pelvis, [0.0; 3])[1] - point(&f.root, [0.0, HIP - 0.1, 0.0])[1]
    };
    let (mut lo, mut hi) = (9.0f32, -9.0f32);
    for k in 0..100 {
        p.t = k as f32 * 0.05;
        let g = gap(&a, &p);
        (lo, hi) = (lo.min(g), hi.max(g));
    }
    assert!(hi - lo < 0.01, "the rider keeps its seat: {lo} to {hi}");
    // Turning right (+z) at speed: it banks right, broom and all.
    let mut at = [0.0f32; 3];
    for k in 0..120 {
        let a0 = k as f32 / 60.0 * 1.2;
        at = [
            at[0] + a0.cos() * 12.0 / 60.0,
            0.0,
            at[2] + a0.sin() * 12.0 / 60.0,
        ];
        a.step(at, (a0, 0.0), (false, false, false), 1.0 / 60.0);
    }
    let f = wizard(at, a.face.x, &a, &pose(), 1);
    let up = dir(&f.root, [0.0, 1.0, 0.0]);
    let right = [-a.face.x.sin(), 0.0, a.face.x.cos()];
    assert!(geo::dot(up, right) > 0.15, "banked into the turn: {up:?}");
}

#[test]
fn turning_where_it_stands_its_feet_stay_put_then_step_round() {
    let mut a = Anim::default();
    let fps = 60.0;
    let mut planted: [Option<V3>; 2] = [None; 2];
    let mut worst: f32 = 0.0;
    let mut lifted = false;
    for k in 0..120 {
        let aim = (k as f32 / 18.0).min(1.0) * 1.5;
        a.step([0.0; 3], (aim, 0.0), (true, false, false), 1.0 / fps);
        let f = wizard([0.0; 3], a.face.x, &a, &pose(), 1);
        for (side, was) in planted.iter_mut().enumerate() {
            let s = sole(&f, side);
            if s[1] < 0.005 {
                let at = *was.get_or_insert(s);
                worst = worst.max(((s[0] - at[0]).powi(2) + (s[2] - at[2]).powi(2)).sqrt());
            } else {
                lifted = true;
                *was = None;
            }
        }
    }
    assert!(worst < 0.03, "a planted foot slid {worst}");
    assert!(lifted, "it stepped round");
    assert!(
        a.held.iter().all(|h| h.abs() < 0.3),
        "and faces its aim: {a:?}"
    );
}

#[test]
fn a_planted_foot_does_not_slide() {
    // Facing east and running every way (ahead, strafing, backing
    // away, on the diagonals): while a foot bears the weight it stays
    // put on the ground, though the body moves on over it.
    for (vx, vz) in [
        (6.5, 0.0),
        (0.0, 6.0),
        (0.0, -6.0),
        (-4.5, 0.0),
        (4.5, 4.5),
        (-3.5, 3.5),
    ] {
        let mut a = Anim::default();
        let mut at = run(&mut a, [0.0; 3], (vx, vz), 0.0, 90);
        let mut planted: Option<(V3, usize)> = None;
        let mut worst: f32 = 0.0;
        for _ in 0..60 {
            at = run(&mut a, at, (vx, vz), 0.0, 1);
            let f = wizard(at, 0.0, &a, &pose(), 1);
            let left = sole(&f, 0);
            if left[1] < 0.01 {
                match planted {
                    Some((p, n)) => {
                        let d = ((left[0] - p[0]).powi(2) + (left[2] - p[2]).powi(2)).sqrt();
                        worst = worst.max(d);
                        planted = Some((p, n + 1));
                    }
                    None => planted = Some((left, 0)),
                }
            } else if planted.is_some_and(|p| p.1 > 3) {
                break;
            } else {
                planted = None;
            }
        }
        let v = (vx, vz);
        assert!(planted.is_some_and(|p| p.1 > 3), "{v:?}: it plants a foot");
        assert!(worst < 0.12, "{v:?}: and it stays: slid {worst}");
    }
}

#[test]
fn crouched_or_landing_its_feet_stay_on_the_ground() {
    let mut a = Anim::default();
    a.crouch = 1.0;
    let f = wizard([0.0; 3], 0.0, &a, &pose(), 1);
    let head = point(&f.head, [0.0, 0.14, 0.0]);
    assert!(head[1] < 1.3, "crouched low: {head:?}");
    for side in 0..2 {
        assert!(sole(&f, side)[1].abs() < 0.03, "feet down");
    }
}

#[test]
fn running_at_any_pace_nothing_jumps_between_frames() {
    // At 144 frames a second, from a walk to past a sprint: no joint
    // moves (against the body) much faster than the body does, and the
    // hips rise and fall in a wave, never turning sharply; a knee that
    // snaps straight, or hips that drop when a foot is far, flicker.
    for v in [2.0f32, 5.0, 7.0, 10.0, 14.0] {
        let fps = 144.0;
        let mut a = Anim::default();
        let mut p = [0.0f32; 3];
        let mut was: Option<(Vec<V3>, f32, f32)> = None;
        for k in 0..(fps as usize * 3) {
            p[0] += v / fps;
            a.step(p, (0.0, 0.0), (true, false, false), 1.0 / fps);
            let f = wizard(p, 0.0, &a, &pose(), 1);
            let joints: Vec<V3> = (0..2)
                .flat_map(|s| {
                    let hand = point(&f.fore[s], [0.0, -FORE, 0.0]);
                    [sole(&f, s), point(&f.shin[s], [0.0; 3]), hand]
                })
                .map(|j| geo::sub(j, p))
                .collect();
            let hip = point(&f.pelvis, [0.0; 3])[1];
            let mut rise = 0.0;
            if let Some((old, oh, or)) = &was {
                rise = (hip - oh) * fps;
                if k > fps as usize {
                    for (j, o) in joints.iter().zip(old) {
                        let d = geo::dot(geo::sub(*j, *o), geo::sub(*j, *o)).sqrt() * fps;
                        assert!(d < 1.25 * v + 3.0, "{v} m/s: a joint at {d} m/s");
                    }
                    let turn = (rise - or).abs() * fps;
                    // A wave quickens with the pace; the old flicker
                    // turned at some 1,800.
                    let most = 60.0 + 0.5 * v * v;
                    assert!(turn < most, "{v} m/s: the hips turned at {turn} m/s/s");
                }
            }
            was = Some((joints, hip, rise));
        }
    }
}

#[test]
fn an_aim_that_jumps_turns_it_smoothly() {
    // What the crosshair is on can jump (near ground, the sky): the
    // body turns after it, never in a frame.
    let mut a = Anim::default();
    let fps = 144.0;
    let mut last = 0.0;
    for k in 0..300 {
        let aim = if (k / 20) % 2 == 0 { 0.0 } else { 0.3 };
        a.step([0.0; 3], (aim, 0.0), (true, false, false), 1.0 / fps);
        let turn = (a.face.x - last).abs();
        assert!(k == 0 || turn < 0.06, "turned {turn} in a frame");
        last = a.face.x;
    }
}

#[test]
fn sliding_it_sits_low_with_a_leg_out_ahead() {
    let mut a = Anim::default();
    let mut p = [0.0; 3];
    for _ in 0..30 {
        p = [p[0] + 11.0 / 60.0, 0.0, 0.0];
        a.step(p, (0.0, 0.0), (true, true, true), 1.0 / 60.0);
    }
    let f = wizard(p, 0.0, &a, &pose(), 1);
    let hip = point(&f.root, [0.0, HIP, 0.0]);
    let pelvis = point(&f.thigh[1], [0.0; 3]);
    assert!(pelvis[1] < 0.6, "low: {pelvis:?} ({hip:?})");
    let lead = sole(&f, 1);
    assert!(lead[0] - p[0] > 0.4, "the lead foot ahead: {lead:?}");
    assert!(lead[1].abs() < 0.12, "and on the ground: {lead:?}");
}

#[test]
fn strafing_its_legs_go_its_way_and_its_chest_keeps_its_aim() {
    // Facing +x, running right (+z).
    let mut a = Anim::default();
    let at = run(&mut a, [0.0; 3], (0.0, 6.0), 0.0, 60);
    let f = wizard(at, 0.0, &a, &pose(), 1);
    let hips_fwd = dir(&f.pelvis, [1.0, 0.0, 0.0]);
    let chest_fwd = dir(&f.chest, [1.0, 0.0, 0.0]);
    assert!(
        hips_fwd[2] > 0.4 && hips_fwd[2] < 0.7,
        "hips turned partly right: {hips_fwd:?}"
    );
    assert!(chest_fwd[0] > 0.9, "chest ahead: {chest_fwd:?}");
}
