use super::*;
use crate::keys::find;

fn settle(h: &mut Hands, secs: f32) {
    for _ in 0..(secs * 60.0) as usize {
        h.step(1.0 / 60.0);
    }
}

fn flat(a: V3, b: V3) -> f32 {
    len([a[0] - b[0], 0.0, a[2] - b[2]])
}

fn tip(h: &Hands, f: Finger) -> V3 {
    h.pose(f.side).fingers[f.digit][3]
}

#[test]
fn at_rest_every_finger_sits_on_its_home_key() {
    let mut h = Hands::new();
    settle(&mut h, 1.0);
    for (f, code) in keys::HOME {
        let k = find(code).unwrap();
        let at = k.spot(f);
        let t = tip(&h, f);
        assert!(flat(t, at) < 0.006, "{f:?} is {} m off {code}", flat(t, at));
        let over = t[1] - RADII[f.digit] - at[1];
        assert!(
            over > -0.001 && over < 0.004,
            "{f:?} rests {over} m over {code}"
        );
    }
}

#[test]
fn the_joints_keep_the_fingers_lengths() {
    let mut h = Hands::new();
    h.key("KeyY", true);
    h.key("Backspace", true);
    settle(&mut h, 0.4);
    for side in [Side::Left, Side::Right] {
        let p = h.pose(side);
        for (d, lens) in SEGMENTS.iter().enumerate().skip(INDEX) {
            for (s, &want) in lens.iter().enumerate() {
                let l = len(sub(p.fingers[d][s + 1], p.fingers[d][s]));
                assert!((l - want).abs() < 1e-4, "{side:?} {d} {s}: {l}");
            }
        }
        assert!((len(sub(p.elbow, p.wrist)) - FOREARM).abs() < 1e-4);
    }
}

#[test]
fn a_pressed_key_is_pressed_by_its_finger_and_let_go() {
    let mut h = Hands::new();
    settle(&mut h, 0.5);
    let k = keys::layout()
        .iter()
        .position(|k| k.code == "KeyJ")
        .unwrap();
    h.key("KeyJ", true);
    settle(&mut h, 0.2);
    let index = Finger {
        side: Side::Right,
        digit: INDEX,
    };
    let t = tip(&h, index);
    let top = find("KeyJ").unwrap().top();
    assert!(flat(t, top) < 0.006);
    assert!(t[1] - RADII[INDEX] < top[1] - 0.002, "the finger is down");
    assert!((h.depth(k) - KEY_TRAVEL).abs() < 1e-4, "the key is down");
    h.key("KeyJ", false);
    settle(&mut h, 0.6);
    assert!(h.depth(k) < 1e-4, "the key is up");
    assert_eq!(h.hand(Side::Right).duty[INDEX], Duty::Rest);
}

#[test]
fn far_keys_are_reached_with_the_wrist() {
    for code in [
        "Backspace",
        "Escape",
        "F12",
        "ArrowLeft",
        "PageDown",
        "Digit5",
        "Enter",
    ] {
        let mut h = Hands::new();
        settle(&mut h, 0.5);
        let key = find(code).unwrap();
        h.key(code, true);
        settle(&mut h, 0.6);
        let t = tip(&h, key.finger);
        assert!(
            flat(t, key.spot(key.finger)) < 0.009,
            "{code}: {} m short",
            flat(t, key.spot(key.finger))
        );
    }
}

#[test]
fn the_right_hand_takes_the_mouse_and_comes_back_to_type() {
    let mut h = Hands::new();
    settle(&mut h, 0.5);
    for _ in 0..10 {
        h.stir(3.0);
        h.step(1.0 / 60.0);
    }
    settle(&mut h, 1.0);
    assert!(h.on_mouse() > 0.99, "on the mouse: {}", h.on_mouse());
    let wrist = h.pose(Side::Right).wrist;
    assert!(
        flat(wrist, h.mouse.at()) < 0.13,
        "the wrist is by the mouse"
    );
    // Space, with the hand on the mouse, is the left thumb's.
    h.key("Space", true);
    settle(&mut h, 0.1);
    assert!(h.hand(Side::Right).wants_mouse);
    assert_eq!(h.hand(Side::Left).duty[THUMB], Duty::Press(space()));
    h.key("Space", false);
    // A letter of the right hand brings it back.
    h.key("KeyK", true);
    settle(&mut h, 1.0);
    assert!(h.on_mouse() < 0.01);
    h.key("KeyK", false);
    // A mouse moving a pixel now and then does not take it away.
    for _ in 0..120 {
        h.stir(0.2);
        h.step(1.0 / 60.0);
    }
    assert!(!h.hand(Side::Right).wants_mouse);
}

fn space() -> usize {
    keys::layout()
        .iter()
        .position(|k| k.code == "Space")
        .unwrap()
}

#[test]
fn a_click_is_a_finger_on_its_button() {
    let index = Finger {
        side: Side::Right,
        digit: INDEX,
    };
    let mut h = Hands::new();
    h.button(2, true);
    h.button(2, false);
    settle(&mut h, 1.0);
    let up = tip(&h, index);
    h.button(0, true);
    settle(&mut h, 0.3);
    let down = tip(&h, index);
    assert!(h.mouse.left && h.on_mouse() > 0.99);
    assert!(
        down[1] < up[1] - 0.0015,
        "the index goes down: {up:?} {down:?}"
    );
    assert!(flat(down, h.mouse.at()) < MOUSE_H * 1.6);
}

#[test]
fn nonsense_never_breaks_the_hands() {
    let mut h = Hands::new();
    h.key("NoSuchKey", true);
    h.key("", false);
    h.stir(f32::NAN);
    h.stir(1e30);
    h.button(7, true);
    h.step(f32::NAN);
    h.step(1e9);
    h.step(-1.0);
    for code in keys::layout().iter().map(|k| k.code) {
        h.key(code, true);
    }
    settle(&mut h, 0.5);
    for side in [Side::Left, Side::Right] {
        let p = h.pose(side);
        for f in p.fingers.iter().flatten() {
            assert!(f.iter().all(|x| x.is_finite()), "{side:?} {f:?}");
        }
    }
}

#[test]
fn the_mouse_follows_the_cursor_across_the_mat() {
    let (x0, z0) = mouse_for((240.0, 135.0), (480, 270));
    assert!((x0 - MOUSE_X).abs() < 1e-6 && (z0 - MOUSE_Z).abs() < 1e-6);
    let (x1, z1) = mouse_for((480.0, 270.0), (480, 270));
    assert!(x1 > x0 && z1 > z0, "right and toward you");
    assert!(x1 - x0 < 0.1 && z1 - z0 < 0.06, "it stays on the mat");
}
