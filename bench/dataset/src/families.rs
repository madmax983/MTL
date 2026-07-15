//! Parameterized task-family generators.
//!
//! Each family spans an existing tier and carries a **difficulty knob** (the
//! parameter magnitude / input-grid size). The known-good MTL is seeded from the
//! real `bench/corpus` and `bench/tier3` solutions and parameterized where
//! sensible. Every instance ships an adversarial io-contract (0, 1, -1,
//! negatives, empty list, `i64::MIN`/`i64::MAX`) so the oracle checks behavior
//! across boundaries, not one input. The Rust reference uses i64 `checked_*` and
//! Rust `/`,`%` (truncate toward zero) — matching MTL semantics exactly.
//!
//! Families are returned as GROUPS so the driver can round-robin across them,
//! guaranteeing tier/family spread even when the pilot count is small.

use mtl_core::interp::Value;

use crate::{int_list, Expected, IoVector, TaskInstance};

fn vi(n: i64) -> Value {
    Value::Int(n)
}

/// Adversarial 1-input integer grid.
fn grid1() -> Vec<i64> {
    vec![0, 1, -1, 2, -2, 7, -7, 100, -100, i64::MIN, i64::MAX]
}

/// Adversarial 2-input integer grid (includes y==0 and MIN/-1 overflow traps).
fn grid2() -> Vec<(i64, i64)> {
    vec![
        (0, 0),
        (1, 0),
        (0, 1),
        (1, 1),
        (-1, 1),
        (1, -1),
        (-1, -1),
        (7, -7),
        (-7, 7),
        (2, 3),
        (-5, 6),
        (100, -100),
        (i64::MIN, 1),
        (i64::MAX, 1),
        (i64::MIN, -1),
        (i64::MAX, -1),
        (5, 0),
    ]
}

fn io_unary(f: impl Fn(i64) -> Option<i64>) -> Vec<IoVector> {
    grid1()
        .into_iter()
        .map(|n| IoVector {
            input: vec![vi(n)],
            expected: match f(n) {
                Some(v) => Expected::Halt(vec![vi(v)]),
                None => Expected::Fault,
            },
        })
        .collect()
}

fn io_binary(f: impl Fn(i64, i64) -> Option<i64>) -> Vec<IoVector> {
    grid2()
        .into_iter()
        .map(|(x, y)| IoVector {
            input: vec![vi(x), vi(y)],
            expected: match f(x, y) {
                Some(v) => Expected::Halt(vec![vi(v)]),
                None => Expected::Fault,
            },
        })
        .collect()
}

fn diff_of(mag: i64) -> u32 {
    match mag.unsigned_abs() {
        0..=3 => 0,
        4..=12 => 1,
        13..=60 => 2,
        _ => 3,
    }
}

/// arithmetic — affine `[n] -> [a*n + b]`, program `{a}*{b}+`.
fn family_affine(seed: u64) -> Vec<TaskInstance> {
    let mut out = Vec::new();
    let off = (seed % 3) as i64;
    for a in 1..=24i64 {
        for b in 0..=24i64 {
            let a = a + off;
            let prog = format!("{a}*{b}+");
            out.push(TaskInstance {
                family: "arithmetic".into(),
                tier: 0,
                difficulty: diff_of(a + b),
                description: format!("Given an integer n on the stack, compute {a}*n + {b}."),
                io: io_unary(move |n| n.checked_mul(a).and_then(|x| x.checked_add(b))),
                program: prog,
                tier3_task: None,
            });
        }
    }
    out
}

/// arithmetic — square-affine `[n] -> [n*n + b]`, program `:*{b}+`.
fn family_square(_seed: u64) -> Vec<TaskInstance> {
    (0..=24i64)
        .map(|b| TaskInstance {
            family: "arithmetic".into(),
            tier: 0,
            difficulty: diff_of(b),
            description: format!("Given an integer n on the stack, compute n*n + {b}."),
            io: io_unary(move |n| n.checked_mul(n).and_then(|x| x.checked_add(b))),
            program: format!(":*{b}+"),
            tier3_task: None,
        })
        .collect()
}

/// arithmetic — linear combination `[x, y] -> [a*y + b*x]`, program `{a}*~{b}*+`.
fn family_lincomb2(seed: u64) -> Vec<TaskInstance> {
    let mut out = Vec::new();
    let off = (seed % 3) as i64;
    for a in 1..=16i64 {
        for b in 1..=16i64 {
            let a = a + off;
            out.push(TaskInstance {
                family: "arithmetic".into(),
                tier: 0,
                difficulty: diff_of(a + b),
                description: format!(
                    "Given integers x then y on the stack, compute {a}*y + {b}*x."
                ),
                io: io_binary(move |x, y| {
                    y.checked_mul(a)
                        .and_then(|ya| x.checked_mul(b).and_then(|xb| ya.checked_add(xb)))
                }),
                program: format!("{a}*~{b}*+"),
                tier3_task: None,
            });
        }
    }
    out
}

/// arithmetic — the 2-input primitives add/sub/mul/div/mod.
fn family_binops(_seed: u64) -> Vec<TaskInstance> {
    let mk = |name: &str, prog: &str, desc: &str, f: fn(i64, i64) -> Option<i64>| TaskInstance {
        family: name.to_string(),
        tier: 0,
        difficulty: 1,
        description: desc.to_string(),
        io: io_binary(f),
        program: prog.to_string(),
        tier3_task: None,
    };
    vec![
        mk(
            "arithmetic",
            "+",
            "Given integers x then y, compute x + y.",
            |x, y| x.checked_add(y),
        ),
        mk(
            "arithmetic",
            "-",
            "Given integers x then y, compute x - y.",
            |x, y| x.checked_sub(y),
        ),
        mk(
            "arithmetic",
            "*",
            "Given integers x then y, compute x * y.",
            |x, y| x.checked_mul(y),
        ),
        mk(
            "arithmetic",
            "/",
            "Given integers x then y (y may be 0), compute x / y (truncated toward zero).",
            |x, y| if y == 0 { None } else { x.checked_div(y) },
        ),
        mk(
            "arithmetic",
            "%",
            "Given integers x then y (y may be 0), compute x % y (truncated toward zero).",
            |x, y| if y == 0 { None } else { x.checked_rem(y) },
        ),
    ]
}

/// stack-shuffle — pure rearrangements, exhaustive small forms.
fn family_stack(_seed: u64) -> Vec<TaskInstance> {
    let tuples: [[i64; 3]; 3] = [[1, 2, 3], [7, 8, 9], [-1, 0, 5]];
    let mut out = Vec::new();
    // (program, arity, description, permutation applied to the top `arity` items)
    struct S {
        prog: &'static str,
        arity: usize,
        desc: &'static str,
        perm: fn(&[i64]) -> Vec<i64>,
    }
    let shuffles = [
        S {
            prog: ":",
            arity: 1,
            desc: "Duplicate the top item: ( a -- a a ).",
            perm: |s| vec![s[0], s[0]],
        },
        S {
            prog: "_",
            arity: 2,
            desc: "Drop the top item: ( a b -- a ).",
            perm: |s| vec![s[0]],
        },
        S {
            prog: "~",
            arity: 2,
            desc: "Swap the top two items: ( a b -- b a ).",
            perm: |s| vec![s[1], s[0]],
        },
        S {
            prog: "^",
            arity: 2,
            desc: "Copy the second item over the top: ( a b -- a b a ).",
            perm: |s| vec![s[0], s[1], s[0]],
        },
        S {
            prog: "~_",
            arity: 2,
            desc: "Keep only the top item (nip): ( a b -- b ).",
            perm: |s| vec![s[1]],
        },
        S {
            prog: "@",
            arity: 3,
            desc: "Rotate the top three left: ( a b c -- b c a ).",
            perm: |s| vec![s[1], s[2], s[0]],
        },
        S {
            prog: "~@",
            arity: 3,
            desc: "Reverse the top three items: ( a b c -- c b a ).",
            perm: |s| vec![s[2], s[1], s[0]],
        },
        S {
            prog: "::",
            arity: 1,
            desc: "Triplicate the top item: ( a -- a a a ).",
            perm: |s| vec![s[0], s[0], s[0]],
        },
    ];
    for s in shuffles {
        let io: Vec<IoVector> = tuples
            .iter()
            .map(|t| {
                let inp: Vec<i64> = t[3 - s.arity..].to_vec();
                let out: Vec<i64> = (s.perm)(&inp);
                IoVector {
                    input: inp.iter().map(|n| vi(*n)).collect(),
                    expected: Expected::Halt(out.iter().map(|n| vi(*n)).collect()),
                }
            })
            .collect();
        out.push(TaskInstance {
            family: "stack-shuffle".into(),
            tier: 0,
            difficulty: (s.prog.len() as u32).min(3),
            description: s.desc.into(),
            io,
            program: s.prog.into(),
            tier3_task: None,
        });
    }
    out
}

/// predicate — is-zero / is-negative / is-positive / parity, plus parametric
/// `== k` and `< k`.
fn family_predicate(seed: u64) -> Vec<TaskInstance> {
    let mut out = Vec::new();
    let b = |x: bool| if x { 1 } else { 0 };
    out.push(TaskInstance {
        family: "predicate".into(),
        tier: 0,
        difficulty: 0,
        description: "Given an integer n, return 1 if n == 0 else 0.".into(),
        io: io_unary(move |n| Some(b(n == 0))),
        program: "0=".into(),
        tier3_task: None,
    });
    out.push(TaskInstance {
        family: "predicate".into(),
        tier: 0,
        difficulty: 0,
        description: "Given an integer n, return 1 if n < 0 (negative) else 0.".into(),
        io: io_unary(move |n| Some(b(n < 0))),
        program: "0<".into(),
        tier3_task: None,
    });
    out.push(TaskInstance {
        family: "predicate".into(),
        tier: 0,
        difficulty: 1,
        description: "Given an integer n, return 1 if n > 0 (positive) else 0.".into(),
        io: io_unary(move |n| Some(b(n > 0))),
        program: "0~<".into(),
        tier3_task: None,
    });
    out.push(TaskInstance {
        family: "predicate".into(),
        tier: 0,
        difficulty: 1,
        description: "Given an integer n, return 1 if n is even else 0.".into(),
        io: io_unary(move |n| Some(b(n % 2 == 0))),
        program: "2%0=".into(),
        tier3_task: None,
    });
    let off = (seed % 4) as i64;
    for k in 1..=18i64 {
        let k = k + off;
        out.push(TaskInstance {
            family: "predicate".into(),
            tier: 0,
            difficulty: diff_of(k),
            description: format!("Given an integer n, return 1 if n == {k} else 0."),
            io: io_unary(move |n| Some(b(n == k))),
            program: format!("{k}="),
            tier3_task: None,
        });
        out.push(TaskInstance {
            family: "predicate".into(),
            tier: 0,
            difficulty: diff_of(k),
            description: format!("Given an integer n, return 1 if n < {k} else 0."),
            io: io_unary(move |n| Some(b(n < k))),
            program: format!("{k}<"),
            tier3_task: None,
        });
    }
    // comparison of two inputs
    out.push(TaskInstance {
        family: "predicate".into(),
        tier: 0,
        difficulty: 1,
        description: "Given integers x then y, return 1 if x == y else 0.".into(),
        io: io_binary(move |x, y| Some(b(x == y))),
        program: "=".into(),
        tier3_task: None,
    });
    out.push(TaskInstance {
        family: "predicate".into(),
        tier: 0,
        difficulty: 1,
        description: "Given integers x then y, return 1 if x < y else 0.".into(),
        io: io_binary(move |x, y| Some(b(x < y))),
        program: "<".into(),
        tier3_task: None,
    });
    out
}

/// recursion — factorial / sum-to / fib (bounded input grid), gcd / power
/// (two-input). Seeded from the real corpus v0.2/v0.3 solutions.
fn family_recursion(_seed: u64) -> Vec<TaskInstance> {
    let mut out = Vec::new();

    // factorial [1][*]&  (PrimRec): n<=0 -> 1
    let fact = |n: i64| -> Option<i64> {
        if n <= 0 {
            return Some(1);
        }
        let mut acc: i64 = 1;
        for k in 1..=n {
            acc = acc.checked_mul(k)?;
        }
        Some(acc)
    };
    out.push(TaskInstance {
        family: "recursion".into(),
        tier: 0,
        difficulty: 2,
        description: "Given a non-negative integer n, compute n! (0! = 1).".into(),
        io: [-2i64, 0, 1, 2, 3, 5, 6, 10, 12]
            .into_iter()
            .map(|n| IoVector {
                input: vec![vi(n)],
                expected: match fact(n) {
                    Some(v) => Expected::Halt(vec![vi(v)]),
                    None => Expected::Fault,
                },
            })
            .collect(),
        program: "[1][*]&".into(),
        tier3_task: None,
    });

    // sum_to [0][+]&  (PrimRec): sum 0..=n, n<=0 -> 0
    let sum_to = |n: i64| -> Option<i64> {
        if n <= 0 {
            return Some(0);
        }
        let mut acc: i64 = 0;
        for k in 1..=n {
            acc = acc.checked_add(k)?;
        }
        Some(acc)
    };
    out.push(TaskInstance {
        family: "recursion".into(),
        tier: 0,
        difficulty: 2,
        description: "Given an integer n, compute 0 + 1 + ... + n (0 if n <= 0).".into(),
        io: [-3i64, 0, 1, 3, 10, 100]
            .into_iter()
            .map(|n| IoVector {
                input: vec![vi(n)],
                expected: Expected::Halt(vec![vi(sum_to(n).unwrap())]),
            })
            .collect(),
        program: "[0][+]&".into(),
        tier3_task: None,
    });

    // fib  0 1@[~^+]._  (Times)
    let fib = |n: i64| -> i64 {
        let (mut a, mut b) = (0i64, 1i64);
        for _ in 0..n.max(0) {
            let t = a + b;
            a = b;
            b = t;
        }
        a
    };
    out.push(TaskInstance {
        family: "recursion".into(),
        tier: 0,
        difficulty: 2,
        description: "Given a non-negative integer n, compute the n-th Fibonacci number (fib(0)=0, fib(1)=1).".into(),
        io: [0i64, 1, 2, 3, 5, 10, 20, 50]
            .into_iter()
            .map(|n| IoVector {
                input: vec![vi(n)],
                expected: Expected::Halt(vec![vi(fib(n))]),
            })
            .collect(),
        program: "0 1@[~^+]._".into(),
        tier3_task: None,
    });

    // gcd [:0=][_][~^%][]|  (LinRec): gcd(a,0)=a
    let gcd = |mut a: i64, mut b: i64| -> i64 {
        while b != 0 {
            let t = a % b;
            a = b;
            b = t;
        }
        a
    };
    out.push(TaskInstance {
        family: "recursion".into(),
        tier: 0,
        difficulty: 3,
        description:
            "Given integers a then b, compute gcd(a, b) by Euclid's algorithm (gcd(a,0)=a).".into(),
        io: [
            (12i64, 8i64),
            (48, 36),
            (17, 5),
            (0, 5),
            (5, 0),
            (10, 10),
            (100, 60),
        ]
        .into_iter()
        .map(|(a, b)| IoVector {
            input: vec![vi(a), vi(b)],
            expected: Expected::Halt(vec![vi(gcd(a, b))]),
        })
        .collect(),
        program: "[:0=][_][~^%][]|".into(),
        tier3_task: None,
    });

    // times_mul  0~[{c}+].  (Times): compute c*n by adding c, n times.
    // For n <= 0 the loop runs zero times, so the result is 0.
    for c in 1..=8i64 {
        out.push(TaskInstance {
            family: "recursion".into(),
            tier: 0,
            difficulty: diff_of(c),
            description: format!(
                "Given an integer n, compute {c}*n by repeated addition (0 if n <= 0)."
            ),
            // bounded grid: `times` runs n iterations, so a huge n would exhaust
            // fuel rather than overflow — the contract stays within small n.
            io: [0i64, 1, -1, 2, -2, 7, -7, 30, -30]
                .into_iter()
                .map(|n| IoVector {
                    input: vec![vi(n)],
                    expected: if n <= 0 {
                        Expected::Halt(vec![vi(0)])
                    } else {
                        Expected::Halt(vec![vi(c * n)])
                    },
                })
                .collect(),
            program: format!("0~[{c}+]."),
            tier3_task: None,
        });
    }

    // power 1~[^*].~_  (Times): b^e, e>=0
    let pow = |b: i64, e: i64| -> Option<i64> {
        let mut acc: i64 = 1;
        for _ in 0..e.max(0) {
            acc = acc.checked_mul(b)?;
        }
        Some(acc)
    };
    out.push(TaskInstance {
        family: "recursion".into(),
        tier: 0,
        difficulty: 3,
        description: "Given integers b then e (e >= 0), compute b raised to the power e (b^0 = 1)."
            .into(),
        io: [
            (2i64, 0i64),
            (2, 3),
            (3, 4),
            (5, 2),
            (2, 10),
            (10, 3),
            (7, 0),
        ]
        .into_iter()
        .map(|(b, e)| IoVector {
            input: vec![vi(b), vi(e)],
            expected: Expected::Halt(vec![vi(pow(b, e).unwrap())]),
        })
        .collect(),
        program: "1~[^*].~_".into(),
        tier3_task: None,
    });

    out
}

/// fold / traversal (tier-2) — sequence folds over a list input, seeded from the
/// corpus v0.3 solutions. Input is a single `Value::Quote` list.
fn family_fold(_seed: u64) -> Vec<TaskInstance> {
    let lists_num: Vec<Vec<i64>> = vec![
        vec![],
        vec![5],
        vec![1, 2, 3],
        vec![-1, -2, -3],
        vec![10, 20, 30],
        vec![0, 0, 0],
    ];
    let lists_nonempty: Vec<Vec<i64>> = vec![
        vec![5],
        vec![3, 1, 4, 1, 5],
        vec![-3, -1, -2],
        vec![7, 7],
        vec![2, 9, 4],
    ];

    let mut out = Vec::new();

    let mk_num = |family: &str,
                  prog: &str,
                  desc: &str,
                  lists: &[Vec<i64>],
                  f: &dyn Fn(&[i64]) -> Value|
     -> TaskInstance {
        TaskInstance {
            family: family.to_string(),
            tier: 2,
            difficulty: 1,
            description: desc.to_string(),
            io: lists
                .iter()
                .map(|l| IoVector {
                    input: vec![int_list(l)],
                    expected: Expected::Halt(vec![f(l)]),
                })
                .collect(),
            program: prog.to_string(),
            tier3_task: None,
        }
    };

    out.push(mk_num(
        "fold",
        "0[+](",
        "Given a list of integers, compute their sum (0 for the empty list).",
        &lists_num,
        &|l| vi(l.iter().sum()),
    ));
    out.push(mk_num(
        "fold",
        "1[*](",
        "Given a list of integers, compute their product (1 for the empty list).",
        &lists_num,
        &|l| vi(l.iter().product()),
    ));
    out.push(mk_num(
        "fold",
        "0[_1+](",
        "Given a list, compute its length.",
        &lists_num,
        &|l| vi(l.len() as i64),
    ));
    out.push(mk_num(
        "fold",
        ">_~[^^<[~_][_]?](",
        "Given a non-empty list of integers, compute the maximum element.",
        &lists_nonempty,
        &|l| vi(*l.iter().max().unwrap()),
    ));
    out.push(mk_num(
        "fold",
        ">_~[^^<[_][~_]?](",
        "Given a non-empty list of integers, compute the minimum element.",
        &lists_nonempty,
        &|l| vi(*l.iter().min().unwrap()),
    ));
    out.push(mk_num(
        "fold",
        "[>0=][0][][$]|",
        "Given a list of integers, compute the bitwise XOR of all elements (0 for the empty list).",
        &lists_num,
        &|l| vi(l.iter().fold(0i64, |a, x| a ^ x)),
    ));

    // reverse_list [][~;]( : output is a Quote
    out.push(TaskInstance {
        family: "fold".into(),
        tier: 2,
        difficulty: 2,
        description: "Given a list of integers, return the list reversed.".into(),
        io: [vec![], vec![1], vec![1, 2, 3], vec![9, 8, 7, 6]]
            .into_iter()
            .map(|l| {
                let mut rev = l.clone();
                rev.reverse();
                IoVector {
                    input: vec![int_list(&l)],
                    expected: Expected::Halt(vec![int_list(&rev)]),
                }
            })
            .collect(),
        program: "[][~;](".into(),
        tier3_task: None,
    });

    out
}

/// glyph-coverage families — apply `!`, dip `'`, cons `;`, cat `,`, xor `$`.
fn family_glyphs(seed: u64) -> Vec<TaskInstance> {
    let mut out = Vec::new();
    let off = (seed % 4) as i64;

    // xor2 `$` : [a, b] -> a ^ b
    out.push(TaskInstance {
        family: "bitwise".into(),
        tier: 2,
        difficulty: 1,
        description: "Given integers a then b, compute their bitwise XOR (a ^ b).".into(),
        io: io_binary(move |a, b| Some(a ^ b)),
        program: "$".into(),
        tier3_task: None,
    });

    // apply_k `[{k}+]!` : [n] -> n + k
    for k in 1..=22i64 {
        let k = k + off;
        out.push(TaskInstance {
            family: "quotation".into(),
            tier: 2,
            difficulty: diff_of(k),
            description: format!(
                "Given an integer n, add {k} to it by applying a quoted increment."
            ),
            io: io_unary(move |n| n.checked_add(k)),
            program: format!("[{k}+]!"),
            tier3_task: None,
        });
    }

    // dip_k `[{k}+]'` : [x, y] -> [x + k, y]
    for k in 1..=16i64 {
        let k = k + off;
        out.push(TaskInstance {
            family: "quotation".into(),
            tier: 2,
            difficulty: diff_of(k),
            description: format!(
                "Given integers x then y, add {k} to x while leaving y on top (using dip)."
            ),
            io: grid2()
                .into_iter()
                .map(|(x, y)| IoVector {
                    input: vec![vi(x), vi(y)],
                    expected: match x.checked_add(k) {
                        Some(v) => Expected::Halt(vec![vi(v), vi(y)]),
                        None => Expected::Fault,
                    },
                })
                .collect(),
            program: format!("[{k}+]'"),
            tier3_task: None,
        });
    }

    // cons_k `[{k}];` : [n] -> list [n, k]
    for k in 1..=16i64 {
        let k = k + off;
        out.push(TaskInstance {
            family: "quotation".into(),
            tier: 2,
            difficulty: diff_of(k),
            description: format!(
                "Given an integer n, build the two-element list containing n then {k}."
            ),
            io: [-3i64, 0, 1, 7, -100]
                .into_iter()
                .map(|n| IoVector {
                    input: vec![vi(n)],
                    expected: Expected::Halt(vec![int_list(&[n, k])]),
                })
                .collect(),
            program: format!("[{k}];"),
            tier3_task: None,
        });
    }

    // append_k `[{k}],` : [list] -> list ++ [k]   (exercises cat `,`)
    for k in 1..=16i64 {
        let k = k + off;
        out.push(TaskInstance {
            family: "quotation".into(),
            tier: 2,
            difficulty: diff_of(k),
            description: format!("Given a list of integers, append the value {k} to its end."),
            io: [vec![], vec![1], vec![1, 2, 3], vec![-5, 0]]
                .into_iter()
                .map(|l| {
                    let mut ll = l.clone();
                    ll.push(k);
                    IoVector {
                        input: vec![int_list(&l)],
                        expected: Expected::Halt(vec![int_list(&ll)]),
                    }
                })
                .collect(),
            program: format!("[{k}],"),
            tier3_task: None,
        });
    }

    // cat2 `,` : [q1, q2] -> q1 ++ q2
    let cat_cases: [(Vec<i64>, Vec<i64>); 4] = [
        (vec![1, 2], vec![3]),
        (vec![], vec![9]),
        (vec![7], vec![]),
        (vec![1], vec![2, 3, 4]),
    ];
    out.push(TaskInstance {
        family: "quotation".into(),
        tier: 2,
        difficulty: 1,
        description: "Given two lists, concatenate them into one list.".into(),
        io: cat_cases
            .iter()
            .map(|(a, b)| {
                let mut cc = a.clone();
                cc.extend(b.iter().copied());
                IoVector {
                    input: vec![int_list(a), int_list(b)],
                    expected: Expected::Halt(vec![int_list(&cc)]),
                }
            })
            .collect(),
        program: ",".into(),
        tier3_task: None,
    });

    out
}

/// tier-3 capability tasks — the 16 known `task_setup` tasks, each seeded with
/// its real `bench/tier3/tasks/<task>/solution.mtl`. Gated via the capability
/// oracle (`task_setup` + `drive`).
fn family_capability(_seed: u64) -> Vec<TaskInstance> {
    // (task_name, seed solution.mtl, English description)
    let tasks: [(&str, &str, &str); 16] = [
        (
            "echo_line",
            "readline emit",
            "Read one input line and emit it unchanged.",
        ),
        (
            "grep_filter",
            "readlines 0[linehit[emit][_]?](_",
            "Read all input lines and emit only those matching the predicate.",
        ),
        (
            "agent_loop",
            "readstate[donep][][step][]|",
            "Run an agent step loop until the done predicate holds.",
        ),
        (
            "json_field",
            "readjson getname emit",
            "Parse the input JSON and emit the value of its name field.",
        ),
        (
            "two_tool_pipeline",
            "readinput fetch parse emit",
            "Read input, fetch, parse, then emit the parsed result.",
        ),
        (
            "retry_on_fault",
            "3[tryop okp][~_][_1-][]|",
            "Retry the operation up to the budget until it succeeds.",
        ),
        (
            "map_lines_tool",
            "readlines 0[transform emit](_",
            "Read all input lines, transform each, and emit the results.",
        ),
        (
            "word_count",
            "readtext tokenize 0[_1+](emitint",
            "Read the input text, tokenize it, and emit the word count.",
        ),
        (
            "transform_hits",
            "readlines 0[linehit[transform emit][_]?](_",
            "Read lines, transform and emit only the matching ones.",
        ),
        (
            "emit_budget",
            "readlines>@emit_>@emit__",
            "Emit the first two input lines within a call budget of two emits.",
        ),
        (
            "guarded_read",
            "[endp][][nextline emit][]|",
            "Read and emit lines until the end predicate holds.",
        ),
        (
            "concat_lines",
            "nextline nextline concat emit",
            "Read two lines, concatenate them, and emit the result.",
        ),
        (
            "select_line",
            "readlines 2 select emit",
            "Read all lines and emit the line at index 2.",
        ),
        (
            "confined_echo",
            "readline emit",
            "With only readline and emit granted, echo one input line.",
        ),
        (
            "confined_grep",
            "readlines 0[linehit[emit][_]?](_",
            "With a restricted grant set, emit only the matching lines.",
        ),
        (
            "budget_grep",
            "readlines 0[linehit[emit][_]?](_",
            "Emit matching lines within an emit call budget of two.",
        ),
    ];
    tasks
        .into_iter()
        .map(|(name, sol, desc)| TaskInstance {
            family: "capability".into(),
            tier: 3,
            difficulty: 2,
            description: desc.to_string(),
            io: Vec::new(),
            program: sol.to_string(),
            tier3_task: Some(name.to_string()),
        })
        .collect()
}

// ===========================================================================
// v0.8 broad-distribution families — the uncovered "sealed" task shapes.
//
// Added for the v0.8 generalization round (see `bench/design-v0.8/`). These
// cover the scan/control-flow and scalar bit/digit shapes the sealed post-mortem
// (`bench/BASELINE-SEALED.md` §6) identified as UNCOVERED by the original
// families — the shapes that dragged out-of-sample compression from ~3.9x to
// ~1.67x. Every program here is oracle-verified by construction (each instance
// ships a Rust `checked_*` reference contract gated by `mtl_core::interp`), and
// each is seeded from a working held-out solution in `bench/sealed/corpus/`
// (the base-`b` digit programs generalise `seal_count_set_bits` / the base-10
// digit programs). NO `Value::Vec` / `Value::Str` — lists are the existing
// tier-2 quote-of-ints, scans use the fold/linrec/cons machinery.
// ===========================================================================

/// A modest scalar grid for digit/bit ops: avoids `i64::MIN` (abs overflow) and
/// keeps products bounded so `*` never overflows on the digit reference.
fn grid_scalar_digits() -> Vec<i64> {
    vec![
        0, 1, 2, 3, 7, 8, 9, 10, 15, 16, 25, 34, 63, 64, 99, 100, 255, 256, 511, 1023, -5, -34,
        -100, -255,
    ]
}

fn io_scalar(grid: &[i64], f: impl Fn(i64) -> Option<i64>) -> Vec<IoVector> {
    grid.iter()
        .map(|&n| IoVector {
            input: vec![vi(n)],
            expected: match f(n) {
                Some(v) => Expected::Halt(vec![vi(v)]),
                None => Expected::Fault,
            },
        })
        .collect()
}

/// A modest list grid for scans (edges: empty, singleton, runs, negatives,
/// alternations). Values stay small so no scan reference overflows.
fn scan_lists() -> Vec<Vec<i64>> {
    vec![
        vec![],
        vec![5],
        vec![3, 8],
        vec![1, 2, 3, 4],
        vec![4, 3, 2, 1],
        vec![3, 1, 4, 1, 5, 9, 2],
        vec![-5, -2, -8, -1],
        vec![7, 7, 7],
        vec![2, 2, 3, 3, 3, 1],
        vec![0, 0, 0],
        vec![-3, 5, -3, 5],
        vec![10, -10, 10, -10, 10],
    ]
}

/// Base-b digit sum of |n| (base 2 == popcount). checked_* reference.
fn digit_sum_base(n: i64, b: i64) -> i64 {
    let mut m = n.unsigned_abs();
    let bb = b as u64;
    let mut s: i64 = 0;
    while m > 0 {
        s += (m % bb) as i64;
        m /= bb;
    }
    s
}

/// Base-b digit product of |n| (n==0 -> 0). `None` on overflow (matches MTL `*`).
fn digit_product_base(n: i64, b: i64) -> Option<i64> {
    if n == 0 {
        return Some(0);
    }
    let mut m = n.unsigned_abs();
    let bb = b as u64;
    let mut p: i64 = 1;
    while m > 0 {
        p = p.checked_mul((m % bb) as i64)?;
        m /= bb;
    }
    Some(p)
}

/// scalar bit/digit ops — popcount, digit-sum base b, digit-product base b.
/// Seeded from `seal_count_set_bits` (base 2) generalised over the base literal.
fn family_bitdigit(_seed: u64) -> Vec<TaskInstance> {
    let mut out = Vec::new();
    let grid = grid_scalar_digits();

    // popcount == digit-sum base 2 (the sealed `seal_count_set_bits` program).
    out.push(TaskInstance {
        family: "bitdigit".into(),
        tier: 0,
        difficulty: 2,
        description: "Given an integer n, count the set bits in |n| (population count).".into(),
        io: io_scalar(&grid, |n| Some(digit_sum_base(n, 2))),
        program: ":0<[0~-][]?0~[:0=][_][:2/~2%@+~][]|".into(),
        tier3_task: None,
    });

    // digit-sum base b for a spread of bases (base 2 already emitted as popcount).
    for b in [3i64, 4, 5, 8, 10, 12, 16] {
        out.push(TaskInstance {
            family: "bitdigit".into(),
            tier: 0,
            difficulty: 2,
            description: format!(
                "Given an integer n, sum the base-{b} digits of |n| (0 for n == 0)."
            ),
            io: io_scalar(&grid, move |n| Some(digit_sum_base(n, b))),
            program: format!(":0<[0~-][]?0~[:0=][_][:{b}/~{b}%@+~][]|"),
            tier3_task: None,
        });
    }

    // digit-product base b (base 10 is `seal_digit_product`; others generalise).
    for b in [8i64, 10, 16] {
        out.push(TaskInstance {
            family: "bitdigit".into(),
            tier: 0,
            difficulty: 2,
            description: format!(
                "Given an integer n, multiply the base-{b} digits of |n| (0 for n == 0)."
            ),
            io: io_scalar(&grid, move |n| digit_product_base(n, b)),
            program: format!(":0<[0~-][]?:0=[_0][1~[:0=][_][:{b}/~{b}%@*~][]|]?"),
            tier3_task: None,
        });
    }

    out
}

/// list-scan / control-flow families — running/adjacent scans over a list.
/// Each program is seeded verbatim from the matching `bench/sealed/corpus/`
/// held-out solution and re-verified here against a fresh `checked_*` reference
/// over a fresh (non-sealed) input grid.
fn family_scan(_seed: u64) -> Vec<TaskInstance> {
    let lists = scan_lists();

    // list -> scalar scans -------------------------------------------------
    let scalar_scan =
        |family: &str, prog: &str, desc: &str, f: &dyn Fn(&[i64]) -> i64| -> TaskInstance {
            TaskInstance {
                family: family.to_string(),
                tier: 2,
                difficulty: 2,
                description: desc.to_string(),
                io: lists
                    .iter()
                    .map(|l| IoVector {
                        input: vec![int_list(l)],
                        expected: Expected::Halt(vec![vi(f(l))]),
                    })
                    .collect(),
                program: prog.to_string(),
                tier3_task: None,
            }
        };

    // list -> list scans ---------------------------------------------------
    let list_scan =
        |family: &str, prog: &str, desc: &str, f: &dyn Fn(&[i64]) -> Vec<i64>| -> TaskInstance {
            TaskInstance {
                family: family.to_string(),
                tier: 2,
                difficulty: 3,
                description: desc.to_string(),
                io: lists
                    .iter()
                    .map(|l| IoVector {
                        input: vec![int_list(l)],
                        expected: Expected::Halt(vec![int_list(&f(l))]),
                    })
                    .collect(),
                program: prog.to_string(),
                tier3_task: None,
            }
        };

    let mut out = vec![
        scalar_scan(
            "scan",
            "[>0=][0][][-]|",
            "Given a list of integers, compute the alternating sum x0 - x1 + x2 - ... (0 for the empty list).",
            &|l| {
                l.iter()
                    .enumerate()
                    .map(|(i, &x)| if i % 2 == 0 { x } else { -x })
                    .sum()
            },
        ),
        scalar_scan(
            "scan",
            "[:>[~_][[]]?>[~_][[]]?>[__0][1]?][_0][:>_>_>__^<@@<*~>_~_][+]|",
            "Given a list of integers, count the strict local maxima (elements strictly greater than both neighbours).",
            &|l| {
                let mut c = 0i64;
                if l.len() >= 3 {
                    for i in 1..l.len() - 1 {
                        if l[i] > l[i - 1] && l[i] > l[i + 1] {
                            c += 1;
                        }
                    }
                }
                c
            },
        ),
        scalar_scan(
            "scan",
            "[:>[~_][[]]?>[__0][1]?][_0][:>_>__-:0<[0~-][]?~>_~_][^^<[~_][_]?]|",
            "Given a list of integers, compute the maximum absolute difference between adjacent elements (0 for lists shorter than 2).",
            &|l| {
                if l.len() < 2 {
                    return 0;
                }
                (1..l.len()).map(|i| (l[i] - l[i - 1]).abs()).max().unwrap()
            },
        ),
        list_scan(
            "scan",
            "[][^>[_^=][0]?[_][~;]?]([][~;](",
            "Given a list of integers, remove consecutive duplicate elements (adjacent dedup).",
            &|l| {
                let mut r: Vec<i64> = Vec::new();
                for &x in l {
                    if r.last() != Some(&x) {
                        r.push(x);
                    }
                }
                r
            },
        ),
        list_scan(
            "scan",
            "[][~>[@:@:>__~[=]'~[~_~1+~;][~[;]'~;1~;]?][[];1~;]?]([][~;](",
            "Given a list of integers, run-length encode adjacent runs, emitting each value followed by its run count.",
            &|l| {
                let mut r: Vec<i64> = Vec::new();
                for &x in l {
                    if r.len() >= 2 && r[r.len() - 2] == x {
                        let n = r.len();
                        r[n - 1] += 1;
                    } else {
                        r.push(x);
                        r.push(1);
                    }
                }
                r
            },
        ),
    ];

    // (start, list) -> scalar : minimum running prefix balance -------------
    let mrb = "^~[>0=][_][[+:[^^<[_][~_]?]']'][]|";
    let mrb_lists = scan_lists();
    for start in [0i64, 5, -3] {
        out.push(TaskInstance {
            family: "scan".into(),
            tier: 2,
            difficulty: 3,
            description: format!(
                "Given a starting balance {start} then a list of deltas, compute the minimum running balance (the lowest the balance ever reaches, including the start)."
            ),
            io: mrb_lists
                .iter()
                .map(|l| {
                    let mut bal = start;
                    let mut lo = start;
                    for &d in l {
                        bal += d;
                        lo = lo.min(bal);
                    }
                    IoVector {
                        input: vec![vi(start), int_list(l)],
                        expected: Expected::Halt(vec![vi(lo)]),
                    }
                })
                .collect(),
            program: mrb.to_string(),
            tier3_task: None,
        });
    }

    out
}

/// All family groups (for round-robin interleaving), in a stable order.
pub fn family_groups(seed: u64) -> Vec<Vec<TaskInstance>> {
    vec![
        family_affine(seed),
        family_lincomb2(seed),
        family_square(seed),
        family_binops(seed),
        family_stack(seed),
        family_predicate(seed),
        family_recursion(seed),
        family_fold(seed),
        family_glyphs(seed),
        family_capability(seed),
        family_bitdigit(seed),
        family_scan(seed),
    ]
}
