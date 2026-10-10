//! The terminal on the monitor: a prompt, what you type into it (with a
//! caret, history and Tab to finish a command), and a handful of
//! commands to try (`help` lists them). It only lays text out in lines
//! of coloured spans; the page draws them.

use std::collections::VecDeque;

use crate::laws::{HISTORY, LINE_MAX, SCROLLBACK};

/// A colour, as the page picks it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ink {
    Text,
    Dim,
    Prompt,
    Accent,
    Good,
    Bad,
    Warm,
}

/// A line: spans of text, each in its colour.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Line(pub Vec<(String, Ink)>);

impl Line {
    pub fn plain(s: &str, ink: Ink) -> Line {
        Line(vec![(s.to_string(), ink)])
    }

    pub fn chars(&self) -> usize {
        self.0.iter().map(|(s, _)| s.chars().count()).sum()
    }

    pub fn text(&self) -> String {
        self.0.iter().map(|(s, _)| s.as_str()).collect()
    }
}

/// What the page knows that a command may tell.
#[derive(Clone, Copy, Debug, Default)]
pub struct Facts<'a> {
    pub gpu: &'a str,
    pub when: &'a str,
    /// Seconds since you sat down.
    pub up: f64,
    pub screen: (i32, i32),
}

/// What a command asks of the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    /// Get up from the desk (let the mouse go).
    StandUp,
    /// Open this address in a new tab.
    Open(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Shell,
    /// Green rain until a key is pressed.
    Matrix,
}

/// What the screen shows: its lines, and where the caret is (row, column)
/// if it shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct View {
    pub lines: Vec<Line>,
    pub caret: Option<(usize, usize)>,
}

pub const COMPUTEHUB: &str = "https://computehub-sigma.vercel.app";
const WHO: &str = "you";
const HOST: &str = "battlestation";
const COMMANDS: [&str; 22] = [
    "help",
    "neofetch",
    "ls",
    "cat",
    "echo",
    "clear",
    "whoami",
    "date",
    "uptime",
    "uname",
    "pwd",
    "cd",
    "history",
    "sudo",
    "matrix",
    "cowsay",
    "fortune",
    "computehub",
    "exit",
    "hello",
    "rm",
    "vim",
];

const FILES: [(&str, &str); 3] = [
    (
        "notes.txt",
        "the keyboard on the desk is yours: every key you press, a finger presses.",
    ),
    (
        "todo.txt",
        "- [x] sit down\n- [x] type something\n- [ ] put the real os on this screen",
    ),
    (".secret", "the hub has more rooms than it shows."),
];

const FORTUNES: [&str; 6] = [
    "there is no cloud. it is just someone else's battlestation.",
    "it works on my desk.",
    "the best keyboard is the one under your hands right now.",
    "touch grass. then come back and touch type.",
    "every pixel here was drawn by rust.",
    "rgb adds 30% more frames. probably.",
];

pub struct Term {
    lines: VecDeque<Line>,
    pub input: String,
    /// The caret, in characters into `input`.
    pub caret: usize,
    history: Vec<String>,
    browse: Option<usize>,
    pub mode: Mode,
    /// Lines the view is scrolled back (Page Up).
    pub scroll: usize,
    ran: u32,
}

impl Default for Term {
    fn default() -> Term {
        Term::new()
    }
}

impl Term {
    pub fn new() -> Term {
        let mut t = Term {
            lines: VecDeque::new(),
            input: String::new(),
            caret: 0,
            history: Vec::new(),
            browse: None,
            mode: Mode::Shell,
            scroll: 0,
            ran: 0,
        };
        t.say("battlestation 0.1 (tty1)", Ink::Dim);
        t.say("", Ink::Text);
        t.say(
            "welcome back. type 'help' to see what this desk can do.",
            Ink::Text,
        );
        t.say("", Ink::Text);
        t
    }

    fn push(&mut self, line: Line) {
        self.lines.push_back(line);
        while self.lines.len() > SCROLLBACK {
            self.lines.pop_front();
        }
    }

    /// Print a line (several, split at newlines).
    pub fn say(&mut self, s: &str, ink: Ink) {
        for l in s.split('\n') {
            self.push(Line::plain(l, ink));
        }
    }

    fn prompt() -> Vec<(String, Ink)> {
        vec![
            (format!("{WHO}@{HOST}"), Ink::Prompt),
            (":".into(), Ink::Text),
            ("~".into(), Ink::Accent),
            ("$ ".into(), Ink::Text),
        ]
    }

    /// Commands run since the terminal opened.
    pub fn ran(&self) -> u32 {
        self.ran
    }

    /// A key (`KeyboardEvent.key`), with Ctrl or not. What the command
    /// asks of the page, if anything.
    pub fn key(&mut self, key: &str, ctrl: bool, facts: &Facts) -> Option<Act> {
        if self.mode == Mode::Matrix {
            self.mode = Mode::Shell;
            return None;
        }
        let n = self.input.chars().count();
        if ctrl {
            match key {
                "l" | "L" => self.lines.clear(),
                "c" | "C" => {
                    let mut l = Line(Term::prompt());
                    l.0.push((format!("{}^C", self.input), Ink::Text));
                    self.push(l);
                    self.set_input(String::new());
                }
                "u" | "U" => self.set_input(String::new()),
                _ => {}
            }
            return None;
        }
        match key {
            "Enter" => return self.enter(facts),
            "Backspace" if self.caret > 0 => {
                self.remove(self.caret - 1);
                self.caret -= 1;
            }
            "Delete" if self.caret < n => self.remove(self.caret),
            "ArrowLeft" => self.caret = self.caret.saturating_sub(1),
            "ArrowRight" => self.caret = (self.caret + 1).min(n),
            "Home" => self.caret = 0,
            "End" => self.caret = n,
            "ArrowUp" => self.recall(true),
            "ArrowDown" => self.recall(false),
            "PageUp" => self.scroll = (self.scroll + 8).min(self.lines.len()),
            "PageDown" => self.scroll = self.scroll.saturating_sub(8),
            "Tab" => self.complete(),
            k if k.chars().count() == 1 && n < LINE_MAX => {
                let c = k.chars().next().unwrap_or(' ');
                if !c.is_control() {
                    let at = self.byte(self.caret);
                    self.input.insert(at, c);
                    self.caret += 1;
                    self.scroll = 0;
                }
            }
            _ => {}
        }
        None
    }

    fn byte(&self, chars: usize) -> usize {
        self.input
            .char_indices()
            .nth(chars)
            .map_or(self.input.len(), |(i, _)| i)
    }

    fn remove(&mut self, chars: usize) {
        let at = self.byte(chars);
        if at < self.input.len() {
            self.input.remove(at);
        }
    }

    fn set_input(&mut self, s: String) {
        self.caret = s.chars().count();
        self.input = s;
    }

    fn recall(&mut self, back: bool) {
        if self.history.is_empty() {
            return;
        }
        let last = self.history.len() - 1;
        self.browse = match (self.browse, back) {
            (None, true) => Some(last),
            (None, false) => None,
            (Some(i), true) => Some(i.saturating_sub(1)),
            (Some(i), false) if i < last => Some(i + 1),
            (Some(_), false) => None,
        };
        let s = self
            .browse
            .map_or(String::new(), |i| self.history[i].clone());
        self.set_input(s);
    }

    fn complete(&mut self) {
        let word = self.input.trim_start();
        if word.is_empty() || word.contains(' ') {
            return;
        }
        let found: Vec<&str> = COMMANDS
            .iter()
            .copied()
            .filter(|c| c.starts_with(word))
            .collect();
        if let [one] = found[..] {
            self.set_input(format!("{one} "));
        } else if found.len() > 1 {
            let mut l = Line(Term::prompt());
            l.0.push((self.input.clone(), Ink::Text));
            self.push(l);
            self.say(&found.join("  "), Ink::Dim);
        }
    }

    fn enter(&mut self, facts: &Facts) -> Option<Act> {
        let line = std::mem::take(&mut self.input);
        self.caret = 0;
        self.browse = None;
        self.scroll = 0;
        let mut echo = Line(Term::prompt());
        echo.0.push((line.clone(), Ink::Text));
        self.push(echo);
        let line = line.trim().to_string();
        if line.is_empty() {
            return None;
        }
        if self.history.last() != Some(&line) {
            self.history.push(line.clone());
            if self.history.len() > HISTORY {
                self.history.remove(0);
            }
        }
        self.ran += 1;
        self.run(&line, facts)
    }

    fn run(&mut self, line: &str, facts: &Facts) -> Option<Act> {
        let (cmd, rest) = line.split_once(' ').unwrap_or((line, ""));
        let rest = rest.trim();
        match cmd {
            "help" => {
                self.say("things to try:", Ink::Text);
                self.say(
                    "  neofetch  ls  cat <file>  echo <words>  fortune  cowsay <words>",
                    Ink::Accent,
                );
                self.say(
                    "  whoami  date  uptime  uname  history  matrix  clear  exit",
                    Ink::Accent,
                );
                self.say(
                    "  computehub   (the os for this screen, in a new tab)",
                    Ink::Accent,
                );
                self.say(
                    "keys: arrows edit and recall, tab finishes, ctrl+l clears.",
                    Ink::Dim,
                );
            }
            "clear" => self.lines.clear(),
            "neofetch" => self.neofetch(facts),
            "ls" => {
                let names: Vec<&str> = FILES
                    .iter()
                    .map(|f| f.0)
                    .filter(|n| !n.starts_with('.') || rest.contains('a'))
                    .collect();
                self.say(&names.join("  "), Ink::Accent);
            }
            "cat" => match FILES.iter().find(|f| f.0 == rest) {
                Some((_, body)) => self.say(body, Ink::Text),
                None if rest.is_empty() => self.say("cat: which file? try 'ls'", Ink::Bad),
                None => self.say(&format!("cat: {rest}: no such file"), Ink::Bad),
            },
            "echo" => self.say(rest, Ink::Text),
            "whoami" => self.say(WHO, Ink::Text),
            "date" => self.say(facts.when, Ink::Text),
            "uptime" => self.say(&format!("up {}", span(facts.up)), Ink::Text),
            "uname" => self.say(
                if rest.contains('a') {
                    "secretspace battlestation 0.1 wasm32-unknown-unknown rust"
                } else {
                    "secretspace"
                },
                Ink::Text,
            ),
            "pwd" => self.say(&format!("/home/{WHO}"), Ink::Text),
            "cd" => self.say("you are already where you need to be.", Ink::Dim),
            "history" => {
                let h: Vec<String> = self
                    .history
                    .iter()
                    .enumerate()
                    .map(|(i, c)| format!("{:>4}  {c}", i + 1))
                    .collect();
                for l in h {
                    self.say(&l, Ink::Text);
                }
            }
            "sudo" => self.say(
                &format!("{WHO} is not in the sudoers file. this incident will be reported."),
                Ink::Bad,
            ),
            "rm" if rest.contains("-rf") => self.say("nice try.", Ink::Bad),
            "vim" | "vi" | "emacs" | "nano" => self.say(
                "you open it. you cannot leave. you close the tab.",
                Ink::Dim,
            ),
            "matrix" => self.mode = Mode::Matrix,
            "cowsay" => self.cowsay(if rest.is_empty() { "moo" } else { rest }),
            "fortune" => {
                let k = (facts.up as usize + self.ran as usize * 7) % FORTUNES.len();
                self.say(FORTUNES[k], Ink::Warm);
            }
            "hello" | "hi" | "hey" | "yo" => self.say("hey :)", Ink::Good),
            "computehub" => {
                self.say("opening computehub in a new tab...", Ink::Good);
                return Some(Act::Open(COMPUTEHUB));
            }
            "exit" | "logout" => {
                self.say("you lean back from the desk.", Ink::Dim);
                return Some(Act::StandUp);
            }
            _ => self.say(&format!("bs: {cmd}: command not found"), Ink::Bad),
        }
        None
    }

    fn neofetch(&mut self, facts: &Facts) {
        const LOGO: [&str; 9] = [
            " .-----------------. ",
            " |  _____________  | ",
            " | |  > _        | | ",
            " | |             | | ",
            " | |_____________| | ",
            " '-------. .-------' ",
            "   ______|_|______   ",
            "  /_=_=_=_=_=_=_=_\\  ",
            "                     ",
        ];
        let info = [
            (format!("{WHO}@{HOST}"), String::new()),
            ("-----------------".into(), String::new()),
            ("os".into(), "secretspace battlestation 0.1".into()),
            ("host".into(), "a desk, at night".into()),
            ("kernel".into(), "wasm32-unknown-unknown".into()),
            ("uptime".into(), span(facts.up)),
            ("shell".into(), "bs".into()),
            (
                "display".into(),
                format!("{}x{}", facts.screen.0, facts.screen.1),
            ),
            ("gpu".into(), facts.gpu.to_string()),
        ];
        for (logo, (k, v)) in LOGO.iter().zip(info) {
            let mut l = Line(vec![
                (logo.to_string(), Ink::Accent),
                ("  ".into(), Ink::Text),
            ]);
            if v.is_empty() {
                l.0.push((k, Ink::Prompt));
            } else {
                l.0.push((format!("{k}: "), Ink::Prompt));
                l.0.push((v, Ink::Text));
            }
            self.push(l);
        }
        let mut swatch = Line(vec![(" ".repeat(23), Ink::Text)]);
        for ink in [
            Ink::Bad,
            Ink::Warm,
            Ink::Good,
            Ink::Accent,
            Ink::Prompt,
            Ink::Text,
        ] {
            swatch.0.push(("###".into(), ink));
        }
        self.push(swatch);
    }

    fn cowsay(&mut self, words: &str) {
        let words: String = words.chars().take(60).collect();
        let n = words.chars().count();
        self.say(&format!(" {}", "_".repeat(n + 2)), Ink::Text);
        self.say(&format!("< {words} >"), Ink::Text);
        self.say(&format!(" {}", "-".repeat(n + 2)), Ink::Text);
        self.say(
            "        \\   ^__^\n         \\  (oo)\\_______\n            (__)\\       )\\/\\\n                ||----w |\n                ||     ||",
            Ink::Text,
        );
    }

    /// The last `rows` lines as a screen `cols` wide shows them (long lines
    /// wrapped), the line being typed last.
    pub fn view(&self, cols: usize, rows: usize) -> View {
        let cols = cols.max(1);
        let mut all: Vec<Line> = Vec::new();
        for l in &self.lines {
            wrap(l, cols, &mut all);
        }
        let mut typing = Line(Term::prompt());
        let before = typing.chars();
        typing.0.push((self.input.clone(), Ink::Text));
        let start = all.len();
        wrap(&typing, cols, &mut all);
        let at = before + self.caret;
        let mut caret = (start + at / cols, at % cols);
        if caret.0 >= all.len() {
            all.push(Line::default());
        }
        let end = all
            .len()
            .saturating_sub(self.scroll.min(all.len().saturating_sub(rows)));
        let first = end.saturating_sub(rows);
        let lines = all[first..end].to_vec();
        let shows = self.scroll == 0 && self.mode == Mode::Shell;
        caret.0 = caret.0.saturating_sub(first);
        View {
            caret: (shows && caret.0 < lines.len()).then_some(caret),
            lines,
        }
    }
}

/// A line cut into lines of at most `cols` characters.
fn wrap(l: &Line, cols: usize, out: &mut Vec<Line>) {
    let mut cur = Line::default();
    let mut n = 0;
    for (s, ink) in &l.0 {
        let mut piece = String::new();
        for c in s.chars() {
            if n == cols {
                if !piece.is_empty() {
                    cur.0.push((std::mem::take(&mut piece), *ink));
                }
                out.push(std::mem::take(&mut cur));
                n = 0;
            }
            piece.push(c);
            n += 1;
        }
        if !piece.is_empty() {
            cur.0.push((piece, *ink));
        }
    }
    out.push(cur);
}

/// A time span as people say it: "2 hours, 5 mins".
pub fn span(secs: f64) -> String {
    let m = (secs.max(0.0) / 60.0) as u64;
    match (m / 60, m % 60) {
        (0, 0) => "less than a minute".into(),
        (0, m) => format!("{m} min{}", if m == 1 { "" } else { "s" }),
        (h, m) => format!("{h} hour{}, {m} mins", if h == 1 { "" } else { "s" }),
    }
}

#[cfg(test)]
mod tests;
