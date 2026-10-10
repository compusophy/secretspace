use super::*;

fn facts() -> Facts<'static> {
    Facts {
        gpu: "test gpu",
        when: "sat oct 10 2026",
        up: 125.0,
        screen: (480, 270),
    }
}

fn typed(t: &mut Term, s: &str) -> Option<Act> {
    let f = facts();
    for c in s.chars() {
        t.key(&c.to_string(), false, &f);
    }
    t.key("Enter", false, &f)
}

fn shown(t: &Term) -> Vec<String> {
    t.view(80, 40).lines.iter().map(Line::text).collect()
}

#[test]
fn echo_prints_what_you_typed() {
    let mut t = Term::new();
    assert_eq!(typed(&mut t, "echo hello desk"), None);
    let s = shown(&t);
    assert!(s
        .iter()
        .any(|l| l == "you@battlestation:~$ echo hello desk"));
    assert!(s.iter().any(|l| l == "hello desk"));
    assert_eq!(t.ran(), 1);
}

#[test]
fn the_caret_edits_in_the_middle() {
    let mut t = Term::new();
    let f = facts();
    for k in [
        "a",
        "c",
        "ArrowLeft",
        "b",
        "End",
        "d",
        "Home",
        "Delete",
        "Backspace",
    ] {
        t.key(k, false, &f);
    }
    assert_eq!(t.input, "bcd");
    assert_eq!(t.caret, 0);
}

#[test]
fn history_comes_back_with_the_arrows() {
    let mut t = Term::new();
    let f = facts();
    typed(&mut t, "whoami");
    typed(&mut t, "date");
    t.key("ArrowUp", false, &f);
    assert_eq!(t.input, "date");
    t.key("ArrowUp", false, &f);
    assert_eq!(t.input, "whoami");
    t.key("ArrowDown", false, &f);
    t.key("ArrowDown", false, &f);
    assert_eq!(t.input, "");
}

#[test]
fn tab_finishes_a_command_and_unknown_ones_are_said() {
    let mut t = Term::new();
    let f = facts();
    for k in ["n", "e", "o", "Tab"] {
        t.key(k, false, &f);
    }
    assert_eq!(t.input, "neofetch ");
    t.key("Enter", false, &f);
    assert!(shown(&t).iter().any(|l| l.contains("gpu: test gpu")));
    typed(&mut t, "frobnicate");
    assert!(shown(&t)
        .iter()
        .any(|l| l == "bs: frobnicate: command not found"));
}

#[test]
fn some_commands_ask_the_page() {
    let mut t = Term::new();
    assert_eq!(typed(&mut t, "exit"), Some(Act::StandUp));
    assert_eq!(typed(&mut t, "computehub"), Some(Act::Open(COMPUTEHUB)));
    typed(&mut t, "matrix");
    assert_eq!(t.mode, Mode::Matrix);
    assert_eq!(t.view(80, 10).caret, None, "no caret in the rain");
    t.key("q", false, &facts());
    assert_eq!(t.mode, Mode::Shell);
    assert_eq!(t.input, "", "the key that ends the rain types nothing");
}

#[test]
fn long_lines_wrap_and_the_caret_follows() {
    let mut t = Term::new();
    let f = facts();
    for _ in 0..30 {
        t.key("x", false, &f);
    }
    let v = t.view(20, 6);
    assert_eq!(v.lines.len(), 6);
    // "you@battlestation:~$ " is 21 characters; 30 more end on the third
    // row of the line being typed.
    let (row, col) = v.caret.unwrap();
    assert_eq!((row, col), (5, (21 + 30) % 20));
    assert!(v.lines.iter().all(|l| l.chars() <= 20));
}

#[test]
fn clear_and_scrollback_are_bounded() {
    let mut t = Term::new();
    for i in 0..(SCROLLBACK + 50) {
        typed(&mut t, &format!("echo {i}"));
    }
    assert!(t.lines.len() <= SCROLLBACK);
    assert!(t.history.len() <= HISTORY);
    typed(&mut t, "clear");
    assert_eq!(t.view(80, 30).lines.len(), 1, "only the prompt");
}

#[test]
fn hostile_keys_never_break_it() {
    let mut t = Term::new();
    let f = facts();
    for k in [
        "",
        "\u{0}",
        "\n",
        "Dead",
        "é",
        "日",
        "🦀",
        "Unidentified",
        "PageUp",
    ] {
        t.key(k, false, &f);
        t.key(k, true, &f);
    }
    for _ in 0..(LINE_MAX + 20) {
        t.key("日", false, &f);
    }
    assert!(t.input.chars().count() <= LINE_MAX);
    t.key("ArrowLeft", false, &f);
    t.key("Backspace", false, &f);
    t.key("Enter", false, &f);
    let v = t.view(0, 0);
    assert!(v.lines.is_empty());
    let _ = t.view(3, 100);
}
