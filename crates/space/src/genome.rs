//! Heredity. A genome is mind source; the line is the gene. A child is its
//! parent's genome, sometimes put through one line-level mutation. The only
//! gate is grammar: a child that does not compile is a miscarriage.

use std::rc::Rc;

use crate::caps::CAPS;
use crate::hash::hash_str;
use crate::mind::{compile, Diag, Program};
use crate::rng::Rng;

/// A compiled genome, shared by every mote that carries it unchanged.
#[derive(Debug)]
pub struct Genome {
    pub src: String,
    pub prog: Program,
    pub hash: u64,
}

impl Genome {
    pub fn new(src: &str) -> Result<Rc<Genome>, Diag> {
        let src = normalize(src);
        let prog = compile(&src, CAPS)?;
        Ok(Rc::new(Genome {
            hash: hash_str(&src),
            src,
            prog,
        }))
    }
}

/// One statement per line, trimmed, blank lines dropped: the canonical form
/// that mutation and hashing see.
pub fn normalize(src: &str) -> String {
    src.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

const COMPARE: &[&str] = &["<=", ">=", "==", "!=", "<", ">"];

/// One mutation: jitter a number, swap a comparison, or duplicate, delete
/// or swap lines. Returns None when the result does not compile.
pub fn mutate(src: &str, rng: &mut Rng) -> Option<Rc<Genome>> {
    let mut lines: Vec<String> = src.lines().map(str::to_string).collect();
    if lines.is_empty() {
        return None;
    }
    let i = rng.below(lines.len() as u64) as usize;
    match rng.below(10) {
        0..=4 => jitter_number(&mut lines[i], rng),
        5..=6 => swap_compare(&mut lines[i], rng),
        7 => {
            let l = lines[i].clone();
            lines.insert(i, l);
        }
        8 if lines.len() > 1 => {
            lines.remove(i);
        }
        _ => {
            let j = rng.below(lines.len() as u64) as usize;
            lines.swap(i, j);
        }
    }
    Genome::new(&lines.join("\n")).ok()
}

/// Spans of ASCII digit runs in a line.
fn number_spans(line: &str) -> Vec<(usize, usize)> {
    let b = line.as_bytes();
    let mut spans = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit()
            && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_'))
        {
            let s = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            spans.push((s, i));
        } else {
            i += 1;
        }
    }
    spans
}

fn jitter_number(line: &mut String, rng: &mut Rng) {
    let spans = number_spans(line);
    if spans.is_empty() {
        return;
    }
    let (s, e) = spans[rng.below(spans.len() as u64) as usize];
    let v: i64 = line[s..e].parse().unwrap_or(0);
    let step = 1 + rng.below((v / 4).max(1) as u64) as i64;
    let nv = if rng.chance(1, 2) {
        v.saturating_add(step)
    } else {
        (v - step).max(0)
    };
    line.replace_range(s..e, &nv.to_string());
}

fn swap_compare(line: &mut String, rng: &mut Rng) {
    let mut found = Vec::new();
    let b = line.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match COMPARE.iter().find(|op| b[i..].starts_with(op.as_bytes())) {
            Some(op) => {
                found.push((i, op.len()));
                i += op.len();
            }
            None => i += 1,
        }
    }
    if found.is_empty() {
        return;
    }
    let (at, len) = found[rng.below(found.len() as u64) as usize];
    let new = COMPARE[rng.below(COMPARE.len() as u64) as usize];
    line.replace_range(at..at + len, new);
}
