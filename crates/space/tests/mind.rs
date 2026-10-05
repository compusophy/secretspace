//! The mind language's guarantees: totality, confinement, coded failures.

use space::caps::CAPS;
use space::mind::{compile, run, Cap, Host, Outcome};

/// A host that records calls and answers a fixed value.
struct Echo {
    calls: Vec<(usize, [i64; 3])>,
    answer: i64,
}

impl Host for Echo {
    fn call(&mut self, cap: usize, args: &[i64; 3]) -> i64 {
        self.calls.push((cap, *args));
        self.answer
    }
}

static TEST_CAPS: &[Cap] = &[
    Cap {
        name: "probe",
        arity: 0,
        params: "",
        cost: 1,
        doc: "",
    },
    Cap {
        name: "say",
        arity: 1,
        params: "v",
        cost: 0,
        doc: "",
    },
    Cap {
        name: "pair",
        arity: 2,
        params: "a,b",
        cost: 5,
        doc: "",
    },
];

fn eval(src: &str, fuel: u64) -> (Vec<(usize, [i64; 3])>, Outcome, u64) {
    let p = compile(src, TEST_CAPS).unwrap_or_else(|d| panic!("{src}: {d}"));
    let mut h = Echo {
        calls: Vec::new(),
        answer: 7,
    };
    let r = run(&p, TEST_CAPS, &mut h, fuel);
    (h.calls, r.outcome, r.used)
}

fn said(src: &str) -> Vec<i64> {
    eval(src, 10_000)
        .0
        .iter()
        .filter(|c| c.0 == 1)
        .map(|c| c.1[0])
        .collect()
}

fn code(src: &str) -> &'static str {
    compile(src, TEST_CAPS).expect_err(src).code
}

#[test]
fn arithmetic_and_logic() {
    assert_eq!(said("say(1 + 2 * 3)"), [7]);
    assert_eq!(said("say((1 + 2) * 3)"), [9]);
    assert_eq!(said("say(7 / 2); say(7 % 2); say(-7 / 2)"), [3, 1, -3]);
    assert_eq!(
        said("say(5 / 0); say(5 % 0)"),
        [0, 0],
        "division by zero is 0, never a fault"
    );
    assert_eq!(
        said("say(1 < 2); say(2 <= 1); say(3 == 3); say(3 != 3)"),
        [1, 0, 1, 0]
    );
    assert_eq!(
        said("say(1 && 0); say(1 || 0); say(!0); say(!5)"),
        [0, 1, 1, 0]
    );
    assert_eq!(said("say(9223372036854775807 + 1)"), [i64::MIN], "wrapping");
}

#[test]
fn variables_are_locals_and_unset_reads_are_zero() {
    assert_eq!(said("let a = 4\na = a + 1\nsay(a)"), [5]);
    assert_eq!(said("say(never)"), [0]);
}

#[test]
fn if_else_and_repeat() {
    assert_eq!(said("if 1 { say(1) } else { say(2) }"), [1]);
    assert_eq!(
        said("if 0 { say(1) } else if 1 { say(2) } else { say(3) }"),
        [2]
    );
    assert_eq!(
        said("let i = 0; repeat 4 { say(i); i = i + 1 }"),
        [0, 1, 2, 3]
    );
    assert_eq!(said("repeat 0 { say(1) }"), Vec::<i64>::new());
}

#[test]
fn short_circuit_skips_calls() {
    let (calls, _, _) = eval("0 && probe()\n1 || probe()", 100);
    assert!(calls.is_empty());
}

#[test]
fn calls_reach_the_host_with_their_args() {
    let (calls, _, _) = eval("pair(3, 4)\nsay(probe())", 100);
    assert_eq!(calls, [(2, [3, 4, 0]), (0, [0, 0, 0]), (1, [7, 0, 0])]);
}

#[test]
fn stop_ends_the_turn() {
    let (calls, outcome, _) = eval("say(1)\nstop\nsay(2)", 100);
    assert_eq!(calls.len(), 1);
    assert_eq!(outcome, Outcome::Stopped);
}

#[test]
fn every_run_halts_within_its_fuel() {
    // The worst a mind can do: nested repeats at the limit.
    let src = "repeat 16 { repeat 16 { repeat 16 { repeat 16 { probe() } } } }";
    for fuel in [0, 1, 10, 1000, 160] {
        let (_, outcome, used) = eval(src, fuel);
        assert!(used <= fuel);
        assert_eq!(outcome, Outcome::OutOfFuel);
    }
    let (_, outcome, used) = eval("say(1)", 1_000);
    assert_eq!(outcome, Outcome::Done);
    assert_eq!(used, 3, "a statement, a call, its argument");
}

#[test]
fn a_call_is_charged_before_the_host_sees_it() {
    // pair costs 5 on top of its step: with 6 fuel the args are evaluated
    // (2) and the call cannot be paid.
    let (calls, outcome, used) = eval("pair(1, 2)", 6);
    assert!(calls.is_empty());
    assert_eq!(outcome, Outcome::OutOfFuel);
    assert_eq!(used, 6);
}

#[test]
fn compile_failures_are_coded() {
    assert_eq!(code("say(1"), "E0101");
    assert_eq!(code("let = 3"), "E0101");
    assert_eq!(code("say(99999999999999999999)"), "E0102");
    assert_eq!(code("say(1) $"), "E0103");
    assert_eq!(code("fly()"), "E0105");
    assert_eq!(code("say(1, 2)"), "E0106");
    assert_eq!(code("repeat 17 { say(1) }"), "E0109");
    assert_eq!(code("repeat n { say(1) }"), "E0109");
    let many: String = (0..17).map(|i| format!("let v{i} = 1\n")).collect();
    assert_eq!(code(&many), "E0107");
    let long: String = (0..41).map(|_| "say(1)\n").collect();
    assert_eq!(code(&long), "E0108");
    assert_eq!(
        code("if 1 { say(1) }\nelse { say(2) }"),
        "E0101",
        "else stays on its if's line"
    );
}

#[test]
fn deep_programs_are_refused_not_overflowed() {
    let chain = vec!["1"; 400].join("+");
    assert_eq!(code(&format!("say({chain})")), "E0104");
    let parens = format!("say({}1{})", "(".repeat(200), ")".repeat(200));
    assert_eq!(code(&parens), "E0104");
    let negs = format!("say({}1)", "-".repeat(200));
    assert_eq!(code(&negs), "E0104");
    let mut nest = String::from("say(1)");
    for _ in 0..30 {
        nest = format!("if 1 {{ {nest} }}");
    }
    assert_eq!(code(&nest), "E0104");
}

#[test]
fn diagnostics_point_at_the_line() {
    let d = compile("say(1)\nsay(2)\nfly(3)", TEST_CAPS).unwrap_err();
    assert_eq!((d.code, d.line), ("E0105", 3));
}

#[test]
fn the_world_table_compiles_every_founder() {
    for (name, src, _) in space::founders::FOUNDERS {
        compile(src, CAPS).unwrap_or_else(|d| panic!("{name}: {d}"));
    }
    compile(space::founders::TEMPLATE, CAPS).expect("template compiles");
}

#[test]
fn garbage_never_panics_the_compiler() {
    let mut rng = space::rng::Rng::new(1);
    let alphabet = b"abcdefghijklmnopqrstuvwxyz0123456789 (){},;=<>!&|+-*/%#\n\tletifelsrpo";
    for _ in 0..4000 {
        let n = rng.below(120) as usize;
        let s: String = (0..n)
            .map(|_| alphabet[rng.below(alphabet.len() as u64) as usize] as char)
            .collect();
        if let Ok(p) = compile(&s, TEST_CAPS) {
            let mut h = Echo {
                calls: Vec::new(),
                answer: 1,
            };
            let r = run(&p, TEST_CAPS, &mut h, 500);
            assert!(r.used <= 500);
        }
    }
}
