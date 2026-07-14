//! Self-contained MTL cons-list stack-machine SPIKE for design-v0.6 option (a).
//!
//! NOT production code. NOT verified. Lives outside the cargo workspace and is
//! compiled with `rustc spike.rs -o spike`. It mirrors the small-step semantics
//! of `crates/mtl-core/src/interp.rs::exec_prim` for exactly the primitives the
//! two target programs use, PLUS two new option-(a) primitives:
//!
//!   nth  `\`  ( [xs] i -- x )   O(n) walk from the head; OOB pushes Int(0)
//!   len  `#`  ( [xs] -- n )     length of the cons-list quotation
//!
//! Value model is unchanged: Int(i64) | Quote(Vec<Word>).
//!
//! Usage:
//!   ./spike 'PROGRAM'  ARG ARG ...
//! where each ARG is either an integer (pushed as Int) or a bracketed quote
//! literal like '[2 7 11 15]' (pushed as Quote). Args are pushed left-to-right,
//! so the last arg ends up on top of the stack. Prints the outcome and final
//! stack.

use std::env;

#[derive(Clone, Debug, PartialEq)]
enum Word {
    PushInt(i64),
    PushQuote(Vec<Word>),
    Prim(char),
}

#[derive(Clone, Debug, PartialEq)]
enum Value {
    Int(i64),
    Quote(Vec<Word>),
}

// ---------------- parser ----------------

fn parse(src: &str) -> Vec<Word> {
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    parse_seq(&chars, &mut i, false)
}

fn parse_seq(cs: &[char], i: &mut usize, in_bracket: bool) -> Vec<Word> {
    let mut out = Vec::new();
    while *i < cs.len() {
        let c = cs[*i];
        if c.is_whitespace() {
            *i += 1;
            continue;
        }
        if c == ']' {
            if in_bracket {
                *i += 1; // consume ]
                return out;
            } else {
                panic!("unexpected ]");
            }
        }
        if c == '[' {
            *i += 1;
            let inner = parse_seq(cs, i, true);
            out.push(Word::PushQuote(inner));
            continue;
        }
        if c.is_ascii_digit() {
            let mut v: i64 = 0;
            while *i < cs.len() && cs[*i].is_ascii_digit() {
                v = v * 10 + (cs[*i] as i64 - '0' as i64);
                *i += 1;
            }
            out.push(Word::PushInt(v));
            continue;
        }
        // any other non-space char is a primitive glyph
        out.push(Word::Prim(c));
        *i += 1;
    }
    if in_bracket {
        panic!("unclosed [");
    }
    out
}

// ---------------- machine ----------------

struct Vm {
    stack: Vec<Value>,
    cont: Vec<Word>,
}

#[derive(Debug)]
enum Outcome {
    Halt,
    Fault(String),
    FuelExhausted,
}

fn value_to_word(v: Value) -> Word {
    match v {
        Value::Int(k) => Word::PushInt(k),
        Value::Quote(q) => Word::PushQuote(q),
    }
}

fn word_to_value(w: Word) -> Option<Value> {
    match w {
        Word::PushInt(k) => Some(Value::Int(k)),
        Word::PushQuote(q) => Some(Value::Quote(q)),
        Word::Prim(_) => None,
    }
}

fn prepend(cont: &mut Vec<Word>, mut prefix: Vec<Word>) {
    if prefix.is_empty() {
        return;
    }
    prefix.append(cont);
    *cont = prefix;
}

fn run(mut vm: Vm, fuel: u64) -> (Outcome, Vec<Value>) {
    let mut steps = 0u64;
    while steps < fuel {
        if vm.cont.is_empty() {
            return (Outcome::Halt, vm.stack);
        }
        steps += 1;
        let w = vm.cont[0].clone();
        match w {
            Word::PushInt(k) => {
                vm.cont.remove(0);
                vm.stack.push(Value::Int(k));
            }
            Word::PushQuote(q) => {
                vm.cont.remove(0);
                vm.stack.push(Value::Quote(q));
            }
            Word::Prim(c) => {
                if let Some(f) = exec_prim(&mut vm, c) {
                    return (Outcome::Fault(f), vm.stack);
                }
            }
        }
    }
    (Outcome::FuelExhausted, vm.stack)
}

/// Execute the primitive at cont[0]. On success mutates vm (and removes the
/// prim word from cont) and returns None. On fault returns Some(reason) and
/// leaves state as-is.
fn exec_prim(vm: &mut Vm, c: char) -> Option<String> {
    let n = vm.stack.len();
    macro_rules! need {
        ($k:expr) => {
            if n < $k {
                return Some(format!("Underflow at `{}`", c));
            }
        };
    }
    macro_rules! pop_quote {
        () => {
            match vm.stack.pop() {
                Some(Value::Quote(q)) => q,
                Some(v) => {
                    vm.stack.push(v);
                    return Some(format!("TypeMismatch (want quote) at `{}`", c));
                }
                None => return Some(format!("Underflow at `{}`", c)),
            }
        };
    }
    match c {
        ':' => {
            // Dup
            need!(1);
            vm.cont.remove(0);
            let t = vm.stack[n - 1].clone();
            vm.stack.push(t);
        }
        '_' => {
            // Drop
            need!(1);
            vm.cont.remove(0);
            vm.stack.pop();
        }
        '~' => {
            // Swap
            need!(2);
            vm.cont.remove(0);
            vm.stack.swap(n - 1, n - 2);
        }
        '@' => {
            // Rot ( a b c -- b c a )
            need!(3);
            vm.cont.remove(0);
            let a = vm.stack.remove(n - 3);
            vm.stack.push(a);
        }
        '^' => {
            // Over ( a b -- a b a )
            need!(2);
            vm.cont.remove(0);
            let a = vm.stack[n - 2].clone();
            vm.stack.push(a);
        }
        '!' => {
            // Apply
            need!(1);
            match &vm.stack[n - 1] {
                Value::Quote(_) => {
                    vm.cont.remove(0);
                    let q = pop_quote!();
                    prepend(&mut vm.cont, q);
                }
                _ => return Some("TypeMismatch at `!`".into()),
            }
        }
        ',' => {
            // Cat ( [a] [b] -- [a b] )
            need!(2);
            match (&vm.stack[n - 2], &vm.stack[n - 1]) {
                (Value::Quote(_), Value::Quote(_)) => {
                    vm.cont.remove(0);
                    let b = pop_quote!();
                    let mut a = pop_quote!();
                    a.extend(b);
                    vm.stack.push(Value::Quote(a));
                }
                _ => return Some("TypeMismatch at `,`".into()),
            }
        }
        ';' => {
            // Cons ( v [q] -- [v q] )
            need!(2);
            match &vm.stack[n - 1] {
                Value::Quote(_) => {
                    vm.cont.remove(0);
                    let q = pop_quote!();
                    let v = vm.stack.pop().unwrap();
                    let mut newq = Vec::with_capacity(q.len() + 1);
                    newq.push(value_to_word(v));
                    newq.extend(q);
                    vm.stack.push(Value::Quote(newq));
                }
                _ => return Some("TypeMismatch at `;`".into()),
            }
        }
        '\'' => {
            // Dip ( a [q] -- ...q... a )
            need!(2);
            match &vm.stack[n - 1] {
                Value::Quote(_) => {
                    vm.cont.remove(0);
                    let q = pop_quote!();
                    let a = vm.stack.pop().unwrap();
                    vm.cont.insert(0, value_to_word(a));
                    prepend(&mut vm.cont, q);
                }
                _ => return Some("TypeMismatch at `'`".into()),
            }
        }
        '+' => return arith(vm, c, |a, b| a.checked_add(b)),
        '-' => return arith(vm, c, |a, b| a.checked_sub(b)),
        '*' => return arith(vm, c, |a, b| a.checked_mul(b)),
        '/' => return divmod(vm, c, true),
        '%' => return divmod(vm, c, false),
        '=' => return cmp(vm, c, |a, b| a == b),
        '<' => return cmp(vm, c, |a, b| a < b),
        '$' => return cmp_xor(vm, c),
        '?' => {
            // If ( c [t] [f] -- ... )
            need!(3);
            match (&vm.stack[n - 3], &vm.stack[n - 2], &vm.stack[n - 1]) {
                (Value::Int(_), Value::Quote(_), Value::Quote(_)) => {
                    vm.cont.remove(0);
                    let f = pop_quote!();
                    let t = pop_quote!();
                    let cc = match vm.stack.pop() {
                        Some(Value::Int(k)) => k,
                        _ => return Some("TypeMismatch at `?`".into()),
                    };
                    let branch = if cc != 0 { t } else { f };
                    prepend(&mut vm.cont, branch);
                }
                _ => return Some("TypeMismatch at `?`".into()),
            }
        }
        '&' => {
            // PrimRec ( n [I] [C] -- r )
            need!(3);
            match (&vm.stack[n - 3], &vm.stack[n - 2], &vm.stack[n - 1]) {
                (Value::Int(_), Value::Quote(_), Value::Quote(_)) => {
                    vm.cont.remove(0);
                    let qc = pop_quote!();
                    let qi = pop_quote!();
                    let k = match vm.stack.pop() {
                        Some(Value::Int(k)) => k,
                        _ => return Some("TypeMismatch at `&`".into()),
                    };
                    if k <= 0 {
                        prepend(&mut vm.cont, qi);
                    } else {
                        let mut recur = Vec::new();
                        recur.push(Word::PushInt(k));
                        recur.push(Word::PushInt(k - 1));
                        recur.push(Word::PushQuote(qi));
                        recur.push(Word::PushQuote(qc.clone()));
                        recur.push(Word::Prim('&'));
                        recur.extend(qc);
                        prepend(&mut vm.cont, recur);
                    }
                }
                _ => return Some("TypeMismatch at `&`".into()),
            }
        }
        '.' => {
            // Times ( n [Q] -- ... )
            need!(2);
            match (&vm.stack[n - 2], &vm.stack[n - 1]) {
                (Value::Int(_), Value::Quote(_)) => {
                    vm.cont.remove(0);
                    let q = pop_quote!();
                    let k = match vm.stack.pop() {
                        Some(Value::Int(k)) => k,
                        _ => return Some("TypeMismatch at `.`".into()),
                    };
                    if k > 0 {
                        let mut recur = q.clone();
                        recur.push(Word::PushInt(k - 1));
                        recur.push(Word::PushQuote(q));
                        recur.push(Word::Prim('.'));
                        prepend(&mut vm.cont, recur);
                    }
                }
                _ => return Some("TypeMismatch at `.`".into()),
            }
        }
        '|' => {
            // LinRec ( [P] [T] [R1] [R2] -- ... )
            need!(4);
            match (
                &vm.stack[n - 4],
                &vm.stack[n - 3],
                &vm.stack[n - 2],
                &vm.stack[n - 1],
            ) {
                (Value::Quote(_), Value::Quote(_), Value::Quote(_), Value::Quote(_)) => {
                    vm.cont.remove(0);
                    let qr2 = pop_quote!();
                    let qr1 = pop_quote!();
                    let qt = pop_quote!();
                    let qp = pop_quote!();
                    let mut else_q = qr1.clone();
                    else_q.push(Word::PushQuote(qp.clone()));
                    else_q.push(Word::PushQuote(qt.clone()));
                    else_q.push(Word::PushQuote(qr1));
                    else_q.push(Word::PushQuote(qr2.clone()));
                    else_q.push(Word::Prim('|'));
                    else_q.extend(qr2);
                    let mut spliced = qp;
                    spliced.push(Word::PushQuote(qt));
                    spliced.push(Word::PushQuote(else_q));
                    spliced.push(Word::Prim('?'));
                    prepend(&mut vm.cont, spliced);
                }
                _ => return Some("TypeMismatch at `|`".into()),
            }
        }
        '>' => {
            // Uncons ( [w...] -- w [...] 1 ) | ( [] -- 0 )
            need!(1);
            match &vm.stack[n - 1] {
                Value::Quote(q) => {
                    if let Some(head) = q.first() {
                        match head {
                            Word::PushInt(_) | Word::PushQuote(_) => {}
                            _ => return Some("TypeMismatch at `>` (bad head)".into()),
                        }
                    }
                }
                _ => return Some("TypeMismatch at `>`".into()),
            }
            vm.cont.remove(0);
            let q = pop_quote!();
            let mut it = q.into_iter();
            match it.next() {
                None => vm.stack.push(Value::Int(0)),
                Some(head) => {
                    let tail: Vec<Word> = it.collect();
                    let hv = word_to_value(head).unwrap();
                    vm.stack.push(hv);
                    vm.stack.push(Value::Quote(tail));
                    vm.stack.push(Value::Int(1));
                }
            }
        }
        '(' => {
            // Fold ( [seq] init [C] -- r ) left fold
            need!(3);
            match (&vm.stack[n - 3], &vm.stack[n - 1]) {
                (Value::Quote(qs), Value::Quote(_)) => {
                    if let Some(head) = qs.first() {
                        match head {
                            Word::PushInt(_) | Word::PushQuote(_) => {}
                            _ => return Some("TypeMismatch at `(` (bad head)".into()),
                        }
                    }
                }
                _ => return Some("TypeMismatch at `(`".into()),
            }
            vm.cont.remove(0);
            let qc = pop_quote!();
            let init = vm.stack.pop().unwrap();
            let qs = pop_quote!();
            let mut it = qs.into_iter();
            match it.next() {
                None => vm.stack.push(init),
                Some(head) => {
                    let tail: Vec<Word> = it.collect();
                    let mut recur = Vec::new();
                    recur.push(Word::PushQuote(tail));
                    recur.push(value_to_word(init));
                    recur.push(head);
                    recur.extend(qc.clone());
                    recur.push(Word::PushQuote(qc));
                    recur.push(Word::Prim('('));
                    prepend(&mut vm.cont, recur);
                }
            }
        }
        // ---------------- v0.6 option (a) NEW primitives ----------------
        '\\' => {
            // nth ( [xs] i -- x ) O(n) walk from the head. OOB pushes Int(0).
            need!(2);
            match (&vm.stack[n - 2], &vm.stack[n - 1]) {
                (Value::Quote(_), Value::Int(_)) => {
                    vm.cont.remove(0);
                    let i = match vm.stack.pop() {
                        Some(Value::Int(k)) => k,
                        _ => unreachable!(),
                    };
                    let xs = pop_quote!();
                    // O(n) walk: index i into the cons-list.
                    let elem = if i >= 0 && (i as usize) < xs.len() {
                        word_to_value(xs[i as usize].clone())
                    } else {
                        None
                    };
                    match elem {
                        Some(v) => vm.stack.push(v),
                        None => vm.stack.push(Value::Int(0)), // flagged OOB default
                    }
                }
                _ => return Some("TypeMismatch at `\\` (nth)".into()),
            }
        }
        '#' => {
            // len ( [xs] -- n )
            need!(1);
            match &vm.stack[n - 1] {
                Value::Quote(_) => {
                    vm.cont.remove(0);
                    let xs = pop_quote!();
                    vm.stack.push(Value::Int(xs.len() as i64));
                }
                _ => return Some("TypeMismatch at `#` (len)".into()),
            }
        }
        other => return Some(format!("unknown glyph `{}`", other)),
    }
    None
}

fn arith(vm: &mut Vm, c: char, op: fn(i64, i64) -> Option<i64>) -> Option<String> {
    let n = vm.stack.len();
    if n < 2 {
        return Some(format!("Underflow at `{}`", c));
    }
    match (&vm.stack[n - 2], &vm.stack[n - 1]) {
        (Value::Int(a), Value::Int(b)) => match op(*a, *b) {
            Some(r) => {
                vm.cont.remove(0);
                vm.stack.truncate(n - 2);
                vm.stack.push(Value::Int(r));
                None
            }
            None => Some(format!("Overflow at `{}`", c)),
        },
        _ => Some(format!("TypeMismatch at `{}`", c)),
    }
}

fn divmod(vm: &mut Vm, c: char, is_div: bool) -> Option<String> {
    let n = vm.stack.len();
    if n < 2 {
        return Some(format!("Underflow at `{}`", c));
    }
    match (&vm.stack[n - 2], &vm.stack[n - 1]) {
        (Value::Int(a), Value::Int(b)) => {
            let (a, b) = (*a, *b);
            if b == 0 {
                return Some(format!("DivByZero at `{}`", c));
            }
            let r = if is_div { a.checked_div(b) } else { a.checked_rem(b) };
            match r {
                Some(r) => {
                    vm.cont.remove(0);
                    vm.stack.truncate(n - 2);
                    vm.stack.push(Value::Int(r));
                    None
                }
                None => Some(format!("Overflow at `{}`", c)),
            }
        }
        _ => Some(format!("TypeMismatch at `{}`", c)),
    }
}

fn cmp(vm: &mut Vm, c: char, op: fn(i64, i64) -> bool) -> Option<String> {
    let n = vm.stack.len();
    if n < 2 {
        return Some(format!("Underflow at `{}`", c));
    }
    match (&vm.stack[n - 2], &vm.stack[n - 1]) {
        (Value::Int(a), Value::Int(b)) => {
            let r = if op(*a, *b) { 1 } else { 0 };
            vm.cont.remove(0);
            vm.stack.truncate(n - 2);
            vm.stack.push(Value::Int(r));
            None
        }
        _ => Some(format!("TypeMismatch at `{}`", c)),
    }
}

fn cmp_xor(vm: &mut Vm, c: char) -> Option<String> {
    let n = vm.stack.len();
    if n < 2 {
        return Some(format!("Underflow at `{}`", c));
    }
    match (&vm.stack[n - 2], &vm.stack[n - 1]) {
        (Value::Int(a), Value::Int(b)) => {
            let r = a ^ b;
            vm.cont.remove(0);
            vm.stack.truncate(n - 2);
            vm.stack.push(Value::Int(r));
            None
        }
        _ => Some(format!("TypeMismatch at `{}`", c)),
    }
}

// ---------------- arg parsing / main ----------------

fn parse_arg(s: &str) -> Value {
    let t = s.trim();
    if t.starts_with('[') {
        let words = parse(t);
        match words.into_iter().next() {
            Some(Word::PushQuote(q)) => Value::Quote(q),
            _ => panic!("bad quote arg: {}", s),
        }
    } else {
        Value::Int(t.parse::<i64>().expect("bad int arg"))
    }
}

fn fmt_value(v: &Value) -> String {
    match v {
        Value::Int(k) => k.to_string(),
        Value::Quote(q) => {
            let mut s = String::from("[");
            for (i, w) in q.iter().enumerate() {
                if i > 0 {
                    s.push(' ');
                }
                match w {
                    Word::PushInt(k) => s.push_str(&k.to_string()),
                    Word::PushQuote(_) => s.push_str("[..]"),
                    Word::Prim(c) => s.push(*c),
                }
            }
            s.push(']');
            s
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: spike 'PROGRAM' [ARG ...]");
        std::process::exit(2);
    }
    // If the program arg starts with '@', read the program text from that file
    // (avoids shell-quoting issues with glyphs like ~ * ` \ etc.).
    let prog_src = if let Some(path) = args[1].strip_prefix('@') {
        std::fs::read_to_string(path).expect("cannot read program file")
    } else {
        args[1].clone()
    };
    let program = parse(prog_src.trim());
    let mut stack = Vec::new();
    for a in &args[2..] {
        stack.push(parse_arg(a));
    }
    let vm = Vm { stack, cont: program };
    let (outcome, final_stack) = run(vm, 10_000_000);
    let rendered: Vec<String> = final_stack.iter().map(fmt_value).collect();
    match outcome {
        Outcome::Halt => println!("HALT  stack=[{}]", rendered.join(" ")),
        Outcome::Fault(f) => println!("FAULT {}  stack=[{}]", f, rendered.join(" ")),
        Outcome::FuelExhausted => println!("FUEL  stack=[{}]", rendered.join(" ")),
    }
}
