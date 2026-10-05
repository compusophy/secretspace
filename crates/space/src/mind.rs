//! The mind language: what a mote thinks in.
//!
//! Small on purpose, so its guarantees are mechanical:
//! - **Total.** No unbounded loops: `repeat N` takes a literal N ≤ MAX_REPEAT,
//!   and every step burns fuel, so every run halts within its tank.
//! - **Confined.** A mind touches the world only through the capability table
//!   it was compiled against. There is no other effect.
//! - **Integer.** i64 only, wrapping; division by zero is 0. Nothing faults.
//!
//! One statement per line: the line is the gene. Compound statements
//! (`if ... { ... }`) stay on one line so mutation can treat them whole.
//! Variables are locals that start at 0 every tick; `load`/`store` keep
//! memory across ticks. Reading a name never set is 0.

use crate::laws::{MAX_DEPTH, MAX_LINES, MAX_REPEAT, MAX_SOURCE, MAX_VARS};

/// One capability a mind may call: the complete effect surface is a table
/// of these.
#[derive(Debug)]
pub struct Cap {
    pub name: &'static str,
    pub arity: usize,
    /// Parameter names, for the card and the editor.
    pub params: &'static str,
    /// Fuel burned per call, on top of the call's own step.
    pub cost: u64,
    pub doc: &'static str,
}

/// The world side of a call. Never fails: impossible acts return a
/// sentinel (0 or -1), never an error.
pub trait Host {
    fn call(&mut self, cap: usize, args: &[i64; 3]) -> i64;
}

/// A coded, located diagnostic. Every failure to compile is one of these.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diag {
    pub code: &'static str,
    pub line: u32,
    pub col: u32,
    pub msg: String,
}

impl std::fmt::Display for Diag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}:{} {}", self.code, self.line, self.col, self.msg)
    }
}

/// A compiled mind.
#[derive(Debug)]
pub struct Program {
    stmts: Vec<Stmt>,
    /// Names of the variables, by slot.
    pub vars: Vec<String>,
}

#[derive(Debug)]
enum Stmt {
    Set(u8, Expr),
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    Repeat(u32, Vec<Stmt>),
    Do(Expr),
    Stop,
}

#[derive(Debug)]
enum Expr {
    Num(i64),
    Var(u8),
    Call(u8, Vec<Expr>),
    Neg(Box<Expr>),
    Not(Box<Expr>),
    Bin(Op, Box<Expr>, Box<Expr>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Op {
    Mul,
    Div,
    Rem,
    Add,
    Sub,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    And,
    Or,
}

// ---- lexing ----------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(i64),
    Ident(String),
    Let,
    If,
    Else,
    Repeat,
    Stop,
    P(&'static str),
    Sep,
    Eof,
}

#[derive(Clone, Debug)]
struct Token {
    tok: Tok,
    line: u32,
    col: u32,
}

const PUNCT: &[&str] = &[
    "==", "!=", "<=", ">=", "&&", "||", "(", ")", "{", "}", ",", "=", "<", ">", "+", "-", "*", "/",
    "%", "!",
];

fn diag(code: &'static str, line: u32, col: u32, msg: impl Into<String>) -> Diag {
    Diag {
        code,
        line,
        col,
        msg: msg.into(),
    }
}

fn lex(src: &str) -> Result<Vec<Token>, Diag> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let (mut i, mut line, mut col) = (0usize, 1u32, 1u32);
    while i < b.len() {
        let c = b[i];
        let start_col = col;
        if c == b'\n' || c == b';' {
            out.push(Token {
                tok: Tok::Sep,
                line,
                col,
            });
            i += 1;
            if c == b'\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
            continue;
        }
        if c == b' ' || c == b'\t' || c == b'\r' {
            i += 1;
            col += 1;
            continue;
        }
        if c == b'#' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if c.is_ascii_digit() {
            let s = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            let text = &src[s..i];
            let n: i64 = text
                .parse()
                .map_err(|_| diag("E0102", line, start_col, "number too large"))?;
            col += (i - s) as u32;
            out.push(Token {
                tok: Tok::Num(n),
                line,
                col: start_col,
            });
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'_' {
            let s = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            col += (i - s) as u32;
            let word = &src[s..i];
            let tok = match word {
                "let" => Tok::Let,
                "if" => Tok::If,
                "else" => Tok::Else,
                "repeat" => Tok::Repeat,
                "stop" => Tok::Stop,
                _ => Tok::Ident(word.to_string()),
            };
            out.push(Token {
                tok,
                line,
                col: start_col,
            });
            continue;
        }
        match PUNCT.iter().find(|p| b[i..].starts_with(p.as_bytes())) {
            Some(p) => {
                i += p.len();
                col += p.len() as u32;
                out.push(Token {
                    tok: Tok::P(p),
                    line,
                    col: start_col,
                });
            }
            None => {
                let ch = src[i..].chars().next().unwrap_or('?');
                return Err(diag(
                    "E0103",
                    line,
                    col,
                    format!("unexpected character '{ch}'"),
                ));
            }
        }
    }
    out.push(Token {
        tok: Tok::Eof,
        line,
        col,
    });
    Ok(out)
}

// ---- parsing ---------------------------------------------------------------

struct Parser<'a> {
    toks: Vec<Token>,
    pos: usize,
    caps: &'a [Cap],
    vars: Vec<String>,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> &Tok {
        &self.toks[self.pos].tok
    }

    fn here(&self) -> (u32, u32) {
        let t = &self.toks[self.pos];
        (t.line, t.col)
    }

    fn bump(&mut self) -> Tok {
        let t = self.toks[self.pos].tok.clone();
        if self.pos + 1 < self.toks.len() {
            self.pos += 1;
        }
        t
    }

    fn err(&self, code: &'static str, msg: impl Into<String>) -> Diag {
        let (l, c) = self.here();
        diag(code, l, c, msg)
    }

    fn expect(&mut self, p: &'static str) -> Result<(), Diag> {
        if *self.peek() == Tok::P(p) {
            self.bump();
            Ok(())
        } else {
            Err(self.err("E0101", format!("expected '{p}'")))
        }
    }

    fn skip_seps(&mut self) {
        while *self.peek() == Tok::Sep {
            self.bump();
        }
    }

    fn slot(&mut self, name: &str) -> Result<u8, Diag> {
        if let Some(i) = self.vars.iter().position(|v| v == name) {
            return Ok(i as u8);
        }
        if self.vars.len() >= MAX_VARS {
            return Err(self.err("E0107", format!("more than {MAX_VARS} variables")));
        }
        self.vars.push(name.to_string());
        Ok((self.vars.len() - 1) as u8)
    }

    /// Statements until `end` (Eof or '}'), separated by newlines or ';'.
    fn stmts(&mut self, depth: u32, in_block: bool) -> Result<Vec<Stmt>, Diag> {
        let mut out = Vec::new();
        self.skip_seps();
        loop {
            match self.peek() {
                Tok::Eof if !in_block => break,
                Tok::P("}") if in_block => break,
                Tok::Eof => return Err(self.err("E0101", "expected '}'")),
                _ => {}
            }
            out.push(self.stmt(depth)?);
            match self.peek() {
                Tok::Sep => self.skip_seps(),
                Tok::Eof | Tok::P("}") => {}
                _ => return Err(self.err("E0101", "expected a new line or ';'")),
            }
        }
        Ok(out)
    }

    fn block(&mut self, depth: u32) -> Result<Vec<Stmt>, Diag> {
        if depth > MAX_DEPTH {
            return Err(self.err("E0104", "nested too deeply"));
        }
        self.expect("{")?;
        let body = self.stmts(depth + 1, true)?;
        self.expect("}")?;
        Ok(body)
    }

    fn stmt(&mut self, depth: u32) -> Result<Stmt, Diag> {
        match self.peek().clone() {
            Tok::Let => {
                self.bump();
                let name = match self.bump() {
                    Tok::Ident(n) => n,
                    _ => return Err(self.err("E0101", "expected a name after 'let'")),
                };
                self.expect("=")?;
                let slot = self.slot(&name)?;
                Ok(Stmt::Set(slot, self.expr(depth)?.0))
            }
            Tok::Ident(name) if self.toks[self.pos + 1].tok == Tok::P("=") => {
                self.bump();
                self.bump();
                let slot = self.slot(&name)?;
                Ok(Stmt::Set(slot, self.expr(depth)?.0))
            }
            Tok::If => {
                self.bump();
                let cond = self.expr(depth)?.0;
                let then = self.block(depth)?;
                let mut other = Vec::new();
                if *self.peek() == Tok::Else {
                    self.bump();
                    if *self.peek() == Tok::If {
                        other.push(self.stmt(depth + 1)?);
                    } else {
                        other = self.block(depth)?;
                    }
                }
                Ok(Stmt::If(cond, then, other))
            }
            Tok::Repeat => {
                self.bump();
                let n = match self.bump() {
                    Tok::Num(n) if (0..=MAX_REPEAT).contains(&n) => n as u32,
                    _ => {
                        return Err(self.err(
                            "E0109",
                            format!("repeat takes a number from 0 to {MAX_REPEAT}"),
                        ))
                    }
                };
                Ok(Stmt::Repeat(n, self.block(depth)?))
            }
            Tok::Stop => {
                self.bump();
                Ok(Stmt::Stop)
            }
            _ => Ok(Stmt::Do(self.expr(depth)?.0)),
        }
    }

    /// An expression and the depth of its tree. The tree's depth is what
    /// the evaluator recurses over, so it is the depth that is capped: a
    /// long `1+1+1+...` chain is as deep as it is long.
    fn expr(&mut self, depth: u32) -> Result<(Expr, u32), Diag> {
        self.binary(0, depth)
    }

    fn binary(&mut self, level: usize, depth: u32) -> Result<(Expr, u32), Diag> {
        const LEVELS: &[&[(&str, Op)]] = &[
            &[("||", Op::Or)],
            &[("&&", Op::And)],
            &[("==", Op::Eq), ("!=", Op::Ne)],
            &[("<", Op::Lt), ("<=", Op::Le), (">", Op::Gt), (">=", Op::Ge)],
            &[("+", Op::Add), ("-", Op::Sub)],
            &[("*", Op::Mul), ("/", Op::Div), ("%", Op::Rem)],
        ];
        if level == LEVELS.len() {
            return self.unary(depth);
        }
        let (mut lhs, mut d) = self.binary(level + 1, depth)?;
        loop {
            let op = match self.peek() {
                Tok::P(p) => LEVELS[level]
                    .iter()
                    .find(|(s, _)| s == p)
                    .map(|&(_, op)| op),
                _ => None,
            };
            let Some(op) = op else { break };
            self.bump();
            let (rhs, rd) = self.binary(level + 1, depth)?;
            d = d.max(rd) + 1;
            if depth + d > MAX_DEPTH {
                return Err(self.err("E0104", "expression nested too deeply"));
            }
            lhs = Expr::Bin(op, Box::new(lhs), Box::new(rhs));
        }
        Ok((lhs, d))
    }

    fn unary(&mut self, depth: u32) -> Result<(Expr, u32), Diag> {
        if depth > MAX_DEPTH {
            return Err(self.err("E0104", "expression nested too deeply"));
        }
        match self.peek() {
            Tok::P("-") => {
                self.bump();
                let (e, d) = self.unary(depth + 1)?;
                Ok((Expr::Neg(Box::new(e)), d + 1))
            }
            Tok::P("!") => {
                self.bump();
                let (e, d) = self.unary(depth + 1)?;
                Ok((Expr::Not(Box::new(e)), d + 1))
            }
            _ => self.primary(depth),
        }
    }

    fn primary(&mut self, depth: u32) -> Result<(Expr, u32), Diag> {
        match self.bump() {
            Tok::Num(n) => Ok((Expr::Num(n), 1)),
            Tok::P("(") => {
                let (e, d) = self.binary(0, depth + 1)?;
                self.expect(")")?;
                Ok((e, d))
            }
            Tok::Ident(name) => {
                if *self.peek() != Tok::P("(") {
                    return Ok((Expr::Var(self.slot(&name)?), 1));
                }
                self.pos -= 1;
                let Some(idx) = self.caps.iter().position(|c| c.name == name) else {
                    return Err(self.err("E0105", format!("no capability named '{name}'")));
                };
                self.bump();
                self.bump();
                let mut args = Vec::new();
                let mut d = 1;
                if *self.peek() != Tok::P(")") {
                    loop {
                        let (a, ad) = self.binary(0, depth + 1)?;
                        d = d.max(ad + 1);
                        args.push(a);
                        if *self.peek() == Tok::P(",") {
                            self.bump();
                        } else {
                            break;
                        }
                    }
                }
                self.expect(")")?;
                let cap = &self.caps[idx];
                if args.len() != cap.arity {
                    return Err(self.err(
                        "E0106",
                        format!(
                            "{}() takes {} argument(s), got {}",
                            cap.name,
                            cap.arity,
                            args.len()
                        ),
                    ));
                }
                Ok((Expr::Call(idx as u8, args), d))
            }
            _ => {
                self.pos = self.pos.saturating_sub(1);
                Err(self.err("E0101", "expected a value"))
            }
        }
    }
}

/// Compile a mind against a capability table.
pub fn compile(src: &str, caps: &[Cap]) -> Result<Program, Diag> {
    if src.len() > MAX_SOURCE {
        return Err(diag(
            "E0108",
            1,
            1,
            format!("longer than {MAX_SOURCE} bytes"),
        ));
    }
    let lines = src.lines().filter(|l| !l.trim().is_empty()).count();
    if lines > MAX_LINES {
        return Err(diag("E0108", 1, 1, format!("more than {MAX_LINES} lines")));
    }
    let toks = lex(src)?;
    let mut p = Parser {
        toks,
        pos: 0,
        caps,
        vars: Vec::new(),
    };
    let stmts = p.stmts(0, false)?;
    Ok(Program {
        stmts,
        vars: p.vars,
    })
}

// ---- running ---------------------------------------------------------------

/// How a run ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Ran to the end of the program.
    Done,
    /// Reached `stop`.
    Stopped,
    /// Ran dry mid-thought.
    OutOfFuel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Run {
    pub used: u64,
    pub outcome: Outcome,
}

enum Halt {
    Stop,
    Fuel,
}

struct Eval<'h, H: Host> {
    host: &'h mut H,
    caps: &'h [Cap],
    fuel: u64,
    used: u64,
    vars: [i64; MAX_VARS],
}

impl<H: Host> Eval<'_, H> {
    fn charge(&mut self, n: u64) -> Result<(), Halt> {
        if self.used + n > self.fuel {
            self.used = self.fuel;
            return Err(Halt::Fuel);
        }
        self.used += n;
        Ok(())
    }

    fn block(&mut self, stmts: &[Stmt]) -> Result<(), Halt> {
        for s in stmts {
            self.stmt(s)?;
        }
        Ok(())
    }

    fn stmt(&mut self, s: &Stmt) -> Result<(), Halt> {
        self.charge(1)?;
        match s {
            Stmt::Set(slot, e) => {
                let v = self.expr(e)?;
                self.vars[*slot as usize] = v;
            }
            Stmt::If(c, then, other) => {
                if self.expr(c)? != 0 {
                    self.block(then)?;
                } else {
                    self.block(other)?;
                }
            }
            Stmt::Repeat(n, body) => {
                for _ in 0..*n {
                    self.block(body)?;
                }
            }
            Stmt::Do(e) => {
                self.expr(e)?;
            }
            Stmt::Stop => return Err(Halt::Stop),
        }
        Ok(())
    }

    fn expr(&mut self, e: &Expr) -> Result<i64, Halt> {
        self.charge(1)?;
        Ok(match e {
            Expr::Num(n) => *n,
            Expr::Var(slot) => self.vars[*slot as usize],
            Expr::Neg(x) => self.expr(x)?.wrapping_neg(),
            Expr::Not(x) => (self.expr(x)? == 0) as i64,
            Expr::Call(idx, args) => {
                let mut a = [0i64; 3];
                for (i, arg) in args.iter().enumerate() {
                    a[i] = self.expr(arg)?;
                }
                self.charge(self.caps[*idx as usize].cost)?;
                self.host.call(*idx as usize, &a)
            }
            Expr::Bin(op, l, r) => {
                let a = self.expr(l)?;
                match op {
                    Op::And if a == 0 => return Ok(0),
                    Op::Or if a != 0 => return Ok(1),
                    _ => {}
                }
                let b = self.expr(r)?;
                match op {
                    Op::Mul => a.wrapping_mul(b),
                    Op::Div => a.checked_div(b).unwrap_or(0),
                    Op::Rem => a.checked_rem(b).unwrap_or(0),
                    Op::Add => a.wrapping_add(b),
                    Op::Sub => a.wrapping_sub(b),
                    Op::Lt => (a < b) as i64,
                    Op::Le => (a <= b) as i64,
                    Op::Gt => (a > b) as i64,
                    Op::Ge => (a >= b) as i64,
                    Op::Eq => (a == b) as i64,
                    Op::Ne => (a != b) as i64,
                    Op::And | Op::Or => (b != 0) as i64,
                }
            }
        })
    }
}

/// Run a mind once with `fuel` to burn. Every statement and every value
/// costs 1; a call costs 1 plus its capability's cost, charged before the
/// host sees it.
pub fn run<H: Host>(prog: &Program, caps: &[Cap], host: &mut H, fuel: u64) -> Run {
    let mut ev = Eval {
        host,
        caps,
        fuel,
        used: 0,
        vars: [0; MAX_VARS],
    };
    let outcome = match ev.block(&prog.stmts) {
        Ok(()) => Outcome::Done,
        Err(Halt::Stop) => Outcome::Stopped,
        Err(Halt::Fuel) => Outcome::OutOfFuel,
    };
    Run {
        used: ev.used,
        outcome,
    }
}
