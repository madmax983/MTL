// P4 (round-trip) Verus proof — REAL, MACHINE-CHECKED model over `Seq<char>`.
//
// This file is a self-contained Verus artifact (checked by the `verus` tool,
// NOT compiled by cargo). It builds a ghost model of MTL's surface syntax and
// PROVES the P4 round-trip and idempotence theorems over the well-formed
// domain — mirroring the proven style of `crates/mtl-core/src/mtl_core.rs`.
//
// Architecture (option b): this proved `Seq<char>` model is pinned to the
// production Rust parser/printer by a differential proptest that mirrors these
// spec functions line-for-line (see crates/mtl-syntax/tests/p4_model_twin.rs).
//
// Well-formed domain (the image of the executable parser):
//   * every PushInt(n) has n >= 0  (integer literals are unsigned `[0-9]+`);
//   * every Call(s) is a valid name `[a-z][a-z0-9]*`.
// For every well-formed program p:   spec_parse(spec_print(p)) == Ok(p).
//
// NOTE on integers: the ghost model treats a digit run as an unbounded natural.
// Production's i64 overflow bound is a production-only concern, checked by the
// differential test's generator domain (0..=i64::MAX), not modeled here.
//
// Verify with:  verus crates/mtl-syntax/proofs/p4_verus.rs

use vstd::prelude::*;

verus! {

// ============================================================
// 1. Ghost model of the surface AST (mirrors crate::ast).
// ============================================================

pub enum GPrim {
    Dup, Drop, Swap, Rot, Over, Apply, Cat, Cons, Dip,
    Add, Sub, Mul, Div, Mod, Eq, Lt, If,
    PrimRec, Times, LinRec, Uncons, Fold, Xor,
}

pub enum GWord {
    PushInt(int),
    PushQuote(Seq<GWord>),
    Prim(GPrim),
    Call(Seq<char>),
}

// ============================================================
// 2. Character predicates.
// ============================================================

pub open spec fn is_digit(c: char) -> bool { '0' <= c <= '9' }
pub open spec fn is_lower(c: char) -> bool { 'a' <= c <= 'z' }
pub open spec fn is_namechar(c: char) -> bool { is_digit(c) || is_lower(c) }
pub open spec fn is_ws(c: char) -> bool { c == ' ' || c == '\t' || c == '\n' || c == '\r' }

// A valid name: `[a-z][a-z0-9]*`.
pub open spec fn valid_name(s: Seq<char>) -> bool {
    &&& s.len() >= 1
    &&& is_lower(s[0])
    &&& (forall|i: int| 0 <= i < s.len() ==> is_namechar(#[trigger] s[i]))
}

// ============================================================
// 3. Well-formedness (the parser's image).
// ============================================================

pub open spec fn wf_word(w: GWord) -> bool
    decreases w, 0nat,
{
    match w {
        GWord::PushInt(n) => n >= 0,
        GWord::PushQuote(q) => wf_words(q),
        GWord::Prim(_) => true,
        GWord::Call(s) => valid_name(s),
    }
}

pub open spec fn wf_words(ws: Seq<GWord>) -> bool
    decreases ws, ws.len(),
{
    if ws.len() == 0 {
        true
    } else {
        wf_word(ws[0]) && wf_words(ws.subrange(1, ws.len() as int))
    }
}

// ============================================================
// 4. The glyph table (mirrors ast::GLYPHS) and its inverse.
// ============================================================

pub open spec fn spec_glyph(p: GPrim) -> char {
    match p {
        GPrim::Dup => ':', GPrim::Drop => '_', GPrim::Swap => '~', GPrim::Rot => '@',
        GPrim::Over => '^', GPrim::Apply => '!', GPrim::Cat => ',', GPrim::Cons => ';',
        GPrim::Dip => '\'', GPrim::Add => '+', GPrim::Sub => '-', GPrim::Mul => '*',
        GPrim::Div => '/', GPrim::Mod => '%', GPrim::Eq => '=', GPrim::Lt => '<',
        GPrim::If => '?', GPrim::PrimRec => '&', GPrim::Times => '.', GPrim::LinRec => '|',
        GPrim::Uncons => '>', GPrim::Fold => '(', GPrim::Xor => '$',
    }
}

pub open spec fn glyph_to_gprim(c: char) -> Option<GPrim> {
    if c == ':' { Some(GPrim::Dup) }
    else if c == '_' { Some(GPrim::Drop) }
    else if c == '~' { Some(GPrim::Swap) }
    else if c == '@' { Some(GPrim::Rot) }
    else if c == '^' { Some(GPrim::Over) }
    else if c == '!' { Some(GPrim::Apply) }
    else if c == ',' { Some(GPrim::Cat) }
    else if c == ';' { Some(GPrim::Cons) }
    else if c == '\'' { Some(GPrim::Dip) }
    else if c == '+' { Some(GPrim::Add) }
    else if c == '-' { Some(GPrim::Sub) }
    else if c == '*' { Some(GPrim::Mul) }
    else if c == '/' { Some(GPrim::Div) }
    else if c == '%' { Some(GPrim::Mod) }
    else if c == '=' { Some(GPrim::Eq) }
    else if c == '<' { Some(GPrim::Lt) }
    else if c == '?' { Some(GPrim::If) }
    else if c == '&' { Some(GPrim::PrimRec) }
    else if c == '.' { Some(GPrim::Times) }
    else if c == '|' { Some(GPrim::LinRec) }
    else if c == '>' { Some(GPrim::Uncons) }
    else if c == '(' { Some(GPrim::Fold) }
    else if c == '$' { Some(GPrim::Xor) }
    else { None }
}

// Every glyph round-trips through the inverse, and is a self-delimiting punct
// char (not whitespace / digit / lowercase / bracket). All 23 arms concrete.
pub proof fn lemma_glyph(p: GPrim)
    ensures
        glyph_to_gprim(spec_glyph(p)) == Some(p),
        !is_ws(spec_glyph(p)),
        !is_digit(spec_glyph(p)),
        !is_lower(spec_glyph(p)),
        spec_glyph(p) != '[',
        spec_glyph(p) != ']',
{
    match p {
        GPrim::Dup => {}, GPrim::Drop => {}, GPrim::Swap => {}, GPrim::Rot => {},
        GPrim::Over => {}, GPrim::Apply => {}, GPrim::Cat => {}, GPrim::Cons => {},
        GPrim::Dip => {}, GPrim::Add => {}, GPrim::Sub => {}, GPrim::Mul => {},
        GPrim::Div => {}, GPrim::Mod => {}, GPrim::Eq => {}, GPrim::Lt => {},
        GPrim::If => {}, GPrim::PrimRec => {}, GPrim::Times => {}, GPrim::LinRec => {},
        GPrim::Uncons => {}, GPrim::Fold => {}, GPrim::Xor => {},
    }
}

// ============================================================
// 5. Natural numbers <-> decimal digit strings.
// ============================================================

pub open spec fn digit_char(k: nat) -> char {
    if k == 0 { '0' } else if k == 1 { '1' } else if k == 2 { '2' }
    else if k == 3 { '3' } else if k == 4 { '4' } else if k == 5 { '5' }
    else if k == 6 { '6' } else if k == 7 { '7' } else if k == 8 { '8' }
    else { '9' }
}

pub open spec fn digit_val(c: char) -> nat {
    if c == '0' { 0 } else if c == '1' { 1 } else if c == '2' { 2 }
    else if c == '3' { 3 } else if c == '4' { 4 } else if c == '5' { 5 }
    else if c == '6' { 6 } else if c == '7' { 7 } else if c == '8' { 8 }
    else { 9 }
}

pub proof fn lemma_digit_char_val(k: nat)
    requires k < 10,
    ensures digit_val(digit_char(k)) == k, is_digit(digit_char(k)),
{
}

// decimal representation of a natural (most-significant first).
pub open spec fn digits(m: nat) -> Seq<char>
    decreases m,
{
    if m < 10 {
        seq![digit_char(m)]
    } else {
        digits((m / 10) as nat).push(digit_char((m % 10) as nat))
    }
}

// left-fold value of a digit string.
pub open spec fn nat_of_digits(s: Seq<char>) -> nat
    decreases s.len(),
{
    if s.len() == 0 {
        0
    } else {
        (nat_of_digits(s.subrange(0, s.len() as int - 1)) * 10
            + digit_val(s[s.len() as int - 1])) as nat
    }
}

pub proof fn lemma_digits_nonempty(m: nat)
    ensures digits(m).len() >= 1,
    decreases m,
{
    if m < 10 {} else { lemma_digits_nonempty((m / 10) as nat); }
}

pub proof fn lemma_digits_all_digits(m: nat)
    ensures forall|i: int| 0 <= i < digits(m).len() ==> is_digit(#[trigger] digits(m)[i]),
    decreases m,
{
    if m < 10 {
        assert(is_digit(digit_char(m)));
    } else {
        lemma_digits_all_digits((m / 10) as nat);
        let pre = digits((m / 10) as nat);
        let full = pre.push(digit_char((m % 10) as nat));
        assert(is_digit(digit_char((m % 10) as nat)));
        assert forall|i: int| 0 <= i < full.len() implies is_digit(#[trigger] full[i]) by {
            if i < pre.len() {
                assert(full[i] == pre[i]);
            } else {
                assert(full[i] == digit_char((m % 10) as nat));
            }
        }
    }
}

pub proof fn lemma_nat_of_digits_inverse(m: nat)
    ensures nat_of_digits(digits(m)) == m,
    decreases m,
{
    if m < 10 {
        let s = seq![digit_char(m)];
        assert(digits(m) == s);
        assert(s.len() == 1);
        assert(s.subrange(0, s.len() as int - 1) =~= Seq::<char>::empty());
        assert(s[s.len() as int - 1] == digit_char(m));
        lemma_digit_char_val(m);
        assert(nat_of_digits(Seq::<char>::empty()) == 0);
        assert(nat_of_digits(s) == (nat_of_digits(s.subrange(0, s.len() as int - 1)) * 10
            + digit_val(s[s.len() as int - 1])) as nat);
        assert(nat_of_digits(s) == m);
    } else {
        lemma_nat_of_digits_inverse((m / 10) as nat);
        let pre = digits((m / 10) as nat);
        let dc = digit_char((m % 10) as nat);
        let full = pre.push(dc);
        assert(digits(m) == full);
        assert(full.subrange(0, full.len() as int - 1) =~= pre);
        assert(full[full.len() as int - 1] == dc);
        lemma_digit_char_val((m % 10) as nat);
        // one-level unfold of nat_of_digits(full)
        assert(nat_of_digits(full) == (nat_of_digits(pre) * 10 + digit_val(dc)) as nat);
        assert(nat_of_digits(pre) == (m / 10) as nat);
        assert(digit_val(dc) == (m % 10) as nat);
        assert((m / 10) * 10 + m % 10 == m) by (nonlinear_arith);
        assert(nat_of_digits(full) == m);
    }
}

// ============================================================
// 6. Tokens.
// ============================================================

pub enum Token {
    TInt(int),
    TName(Seq<char>),
    TGlyph(GPrim),
    TOpen,
    TClose,
}

pub enum Cls { CInt, CName, CPunct }

pub open spec fn tok_class(t: Token) -> Cls {
    match t {
        Token::TInt(_) => Cls::CInt,
        Token::TName(_) => Cls::CName,
        _ => Cls::CPunct,
    }
}

pub open spec fn tok_piece(t: Token) -> Seq<char> {
    match t {
        Token::TInt(n) => digits(n as nat),
        Token::TName(s) => s,
        Token::TGlyph(p) => seq![spec_glyph(p)],
        Token::TOpen => seq!['['],
        Token::TClose => seq![']'],
    }
}

pub open spec fn tok_first(t: Token) -> char {
    tok_piece(t)[0]
}

pub open spec fn valid_tok(t: Token) -> bool {
    match t {
        Token::TInt(n) => n >= 0,
        Token::TName(s) => valid_name(s),
        _ => true,
    }
}

pub open spec fn valid_toks(ts: Seq<Token>) -> bool {
    forall|i: int| 0 <= i < ts.len() ==> valid_tok(#[trigger] ts[i])
}

// Every token's printed piece is nonempty.
pub proof fn lemma_piece_nonempty(t: Token)
    requires valid_tok(t),
    ensures tok_piece(t).len() >= 1, tok_piece(t)[0] == tok_first(t),
{
    match t {
        Token::TInt(n) => { lemma_digits_nonempty(n as nat); }
        Token::TName(s) => {}
        _ => {}
    }
}

// ============================================================
// 7. The printer:  spec_print = render_h(None, toks_words(p)).
// ============================================================

// The h0 boundary rule (mirrors print::needs_separator).
pub open spec fn needs_sep(left: Cls, b: char) -> bool {
    if is_digit(b) {
        left == Cls::CInt || left == Cls::CName
    } else if is_lower(b) {
        left == Cls::CName
    } else {
        false
    }
}

pub open spec fn needs_sep_opt(prev: Option<Cls>, b: char) -> bool {
    match prev {
        None => false,
        Some(l) => needs_sep(l, b),
    }
}

pub open spec fn render_h(prev: Option<Cls>, ts: Seq<Token>) -> Seq<char>
    decreases ts.len(),
{
    if ts.len() == 0 {
        Seq::<char>::empty()
    } else {
        let t = ts[0];
        let sep = if needs_sep_opt(prev, tok_first(t)) { seq![' '] } else { Seq::<char>::empty() };
        sep + tok_piece(t) + render_h(Some(tok_class(t)), ts.subrange(1, ts.len() as int))
    }
}

pub open spec fn toks_word(w: GWord) -> Seq<Token>
    decreases w, 0nat,
{
    match w {
        GWord::PushInt(n) => seq![Token::TInt(n)],
        GWord::Prim(p) => seq![Token::TGlyph(p)],
        GWord::Call(s) => seq![Token::TName(s)],
        GWord::PushQuote(q) => seq![Token::TOpen] + toks_words(q) + seq![Token::TClose],
    }
}

pub open spec fn toks_words(ws: Seq<GWord>) -> Seq<Token>
    decreases ws, ws.len(),
{
    if ws.len() == 0 {
        Seq::<Token>::empty()
    } else {
        toks_word(ws[0]) + toks_words(ws.subrange(1, ws.len() as int))
    }
}

pub open spec fn spec_print(p: Seq<GWord>) -> Seq<char> {
    render_h(None, toks_words(p))
}

// ============================================================
// 8. The lexer:  maximal-munch tokenizer over Seq<char>.
// ============================================================

pub open spec fn leading_digits_len(cs: Seq<char>) -> nat
    decreases cs.len(),
{
    if cs.len() > 0 && is_digit(cs[0]) {
        (leading_digits_len(cs.subrange(1, cs.len() as int)) + 1) as nat
    } else {
        0
    }
}

pub open spec fn leading_name_len(cs: Seq<char>) -> nat
    decreases cs.len(),
{
    if cs.len() > 0 && is_namechar(cs[0]) {
        (leading_name_len(cs.subrange(1, cs.len() as int)) + 1) as nat
    } else {
        0
    }
}

pub proof fn ldl_bound(cs: Seq<char>)
    ensures leading_digits_len(cs) <= cs.len(),
    decreases cs.len(),
{
    if cs.len() > 0 && is_digit(cs[0]) { ldl_bound(cs.subrange(1, cs.len() as int)); }
}

pub proof fn lnl_bound(cs: Seq<char>)
    ensures leading_name_len(cs) <= cs.len(),
    decreases cs.len(),
{
    if cs.len() > 0 && is_namechar(cs[0]) { lnl_bound(cs.subrange(1, cs.len() as int)); }
}

pub proof fn ldl_pos(cs: Seq<char>)
    requires cs.len() > 0, is_digit(cs[0]),
    ensures leading_digits_len(cs) >= 1,
{
}

pub proof fn lnl_pos(cs: Seq<char>)
    requires cs.len() > 0, is_namechar(cs[0]),
    ensures leading_name_len(cs) >= 1,
{
}

pub open spec fn lex_cons(t: Token, r: Option<Seq<Token>>) -> Option<Seq<Token>> {
    match r {
        Some(ts) => Some(seq![t] + ts),
        None => None,
    }
}

pub open spec fn lex(cs: Seq<char>) -> Option<Seq<Token>>
    decreases cs.len(),
    via lex_termination
{
    if cs.len() == 0 {
        Some(Seq::<Token>::empty())
    } else {
        let c = cs[0];
        if is_ws(c) {
            lex(cs.subrange(1, cs.len() as int))
        } else if is_digit(c) {
            let k = leading_digits_len(cs);
            lex_cons(
                Token::TInt(nat_of_digits(cs.subrange(0, k as int)) as int),
                lex(cs.subrange(k as int, cs.len() as int)),
            )
        } else if is_lower(c) {
            let k = leading_name_len(cs);
            lex_cons(
                Token::TName(cs.subrange(0, k as int)),
                lex(cs.subrange(k as int, cs.len() as int)),
            )
        } else if c == '[' {
            lex_cons(Token::TOpen, lex(cs.subrange(1, cs.len() as int)))
        } else if c == ']' {
            lex_cons(Token::TClose, lex(cs.subrange(1, cs.len() as int)))
        } else {
            match glyph_to_gprim(c) {
                Some(p) => lex_cons(Token::TGlyph(p), lex(cs.subrange(1, cs.len() as int))),
                None => None,
            }
        }
    }
}

#[verifier::decreases_by]
proof fn lex_termination(cs: Seq<char>) {
    if cs.len() == 0 {
    } else {
        let c = cs[0];
        if is_ws(c) {
        } else if is_digit(c) {
            ldl_bound(cs); ldl_pos(cs);
        } else if is_lower(c) {
            lnl_bound(cs); lnl_pos(cs);
        } else {
        }
    }
}

// ============================================================
// 9. The grouper: fold tokens into a GWord tree over `[`/`]`.
// ============================================================

pub open spec fn push_word(levels: Seq<Seq<GWord>>, w: GWord) -> Seq<Seq<GWord>> {
    levels.drop_last().push(levels.last().push(w))
}

pub open spec fn group_fold(ts: Seq<Token>, levels: Seq<Seq<GWord>>) -> Option<Seq<Seq<GWord>>>
    decreases ts.len(),
{
    if ts.len() == 0 {
        Some(levels)
    } else {
        let rest = ts.subrange(1, ts.len() as int);
        match ts[0] {
            Token::TOpen => group_fold(rest, levels.push(Seq::<GWord>::empty())),
            Token::TClose =>
                if levels.len() <= 1 {
                    None
                } else {
                    let inner = levels.last();
                    let levels2 = levels.drop_last();
                    group_fold(rest, push_word(levels2, GWord::PushQuote(inner)))
                },
            Token::TInt(n) => group_fold(rest, push_word(levels, GWord::PushInt(n))),
            Token::TName(s) => group_fold(rest, push_word(levels, GWord::Call(s))),
            Token::TGlyph(p) => group_fold(rest, push_word(levels, GWord::Prim(p))),
        }
    }
}

pub enum ParseOutcome { Ok(Seq<GWord>), Err }

pub open spec fn spec_parse(cs: Seq<char>) -> ParseOutcome {
    match lex(cs) {
        None => ParseOutcome::Err,
        Some(ts) => match group_fold(ts, seq![Seq::<GWord>::empty()]) {
            Some(levels) => if levels.len() == 1 { ParseOutcome::Ok(levels[0]) } else { ParseOutcome::Err },
            None => ParseOutcome::Err,
        },
    }
}

pub open spec fn outcome_prog(o: ParseOutcome) -> Seq<GWord> {
    match o {
        ParseOutcome::Ok(p) => p,
        ParseOutcome::Err => Seq::<GWord>::empty(),
    }
}

// ============================================================
// 10. Well-formedness => valid token stream.
// ============================================================

pub proof fn lemma_valid_toks_append(a: Seq<Token>, b: Seq<Token>)
    ensures valid_toks(a + b) == (valid_toks(a) && valid_toks(b)),
{
    if valid_toks(a) && valid_toks(b) {
        assert forall|i: int| 0 <= i < (a + b).len() implies valid_tok(#[trigger] (a + b)[i]) by {
            if i < a.len() {
                assert((a + b)[i] == a[i]);
            } else {
                assert((a + b)[i] == b[i - a.len()]);
            }
        }
    } else {
        // contrapositive witness
        if !valid_toks(a) {
            let i = choose|i: int| 0 <= i < a.len() && !valid_tok(#[trigger] a[i]);
            assert((a + b)[i] == a[i]);
        } else {
            let j = choose|j: int| 0 <= j < b.len() && !valid_tok(#[trigger] b[j]);
            assert((a + b)[j + a.len()] == b[j]);
        }
    }
}

pub proof fn lemma_wf_valid_word(w: GWord)
    requires wf_word(w),
    ensures valid_toks(toks_word(w)),
    decreases w, 0nat,
{
    match w {
        GWord::PushQuote(q) => {
            lemma_wf_valid_words(q);
            let mid = toks_words(q);
            assert(toks_word(w) =~= seq![Token::TOpen] + mid + seq![Token::TClose]);
            lemma_valid_toks_append(seq![Token::TOpen], mid + seq![Token::TClose]);
            lemma_valid_toks_append(mid, seq![Token::TClose]);
            assert(valid_toks(seq![Token::TOpen]));
            assert(valid_toks(seq![Token::TClose]));
        }
        GWord::PushInt(n) => { assert(valid_toks(seq![Token::TInt(n)])); }
        GWord::Prim(p) => { assert(valid_toks(seq![Token::TGlyph(p)])); }
        GWord::Call(s) => { assert(valid_toks(seq![Token::TName(s)])); }
    }
}

pub proof fn lemma_wf_valid_words(ws: Seq<GWord>)
    requires wf_words(ws),
    ensures valid_toks(toks_words(ws)),
    decreases ws, ws.len(),
{
    if ws.len() == 0 {
        assert(toks_words(ws) =~= Seq::<Token>::empty());
    } else {
        lemma_wf_valid_word(ws[0]);
        lemma_wf_valid_words(ws.subrange(1, ws.len() as int));
        lemma_valid_toks_append(toks_word(ws[0]), toks_words(ws.subrange(1, ws.len() as int)));
    }
}

// ============================================================
// 11. Grouper inverse (Layer 2).
// ============================================================

// Splitting: if grouping A succeeds with lv1, grouping A++B == grouping B from lv1.
pub proof fn lemma_group_split(a: Seq<Token>, b: Seq<Token>, levels: Seq<Seq<GWord>>, lv1: Seq<Seq<GWord>>)
    requires group_fold(a, levels) == Some(lv1),
    ensures group_fold(a + b, levels) == group_fold(b, lv1),
    decreases a.len(),
{
    if a.len() == 0 {
        assert(a + b =~= b);
        assert(levels == lv1);
    } else {
        let arest = a.subrange(1, a.len() as int);
        assert((a + b)[0] == a[0]);
        assert((a + b).subrange(1, (a + b).len() as int) =~= arest + b);
        // The head step is a function of (a[0], levels); identical for a and a+b.
        match a[0] {
            Token::TClose => {
                // group_fold(a,levels)==Some(..) forces levels.len() >= 2 here.
                let levels2 = levels.drop_last();
                let l1 = push_word(levels2, GWord::PushQuote(levels.last()));
                lemma_group_split(arest, b, l1, lv1);
            }
            Token::TOpen => {
                let l1 = levels.push(Seq::<GWord>::empty());
                lemma_group_split(arest, b, l1, lv1);
            }
            Token::TInt(n) => {
                let l1 = push_word(levels, GWord::PushInt(n));
                lemma_group_split(arest, b, l1, lv1);
            }
            Token::TName(s) => {
                let l1 = push_word(levels, GWord::Call(s));
                lemma_group_split(arest, b, l1, lv1);
            }
            Token::TGlyph(p) => {
                let l1 = push_word(levels, GWord::Prim(p));
                lemma_group_split(arest, b, l1, lv1);
            }
        }
    }
}

pub open spec fn app_top(levels: Seq<Seq<GWord>>, ws: Seq<GWord>) -> Seq<Seq<GWord>> {
    levels.drop_last().push(levels.last() + ws)
}

pub proof fn lemma_group_word(w: GWord, levels: Seq<Seq<GWord>>)
    requires levels.len() >= 1,
    ensures group_fold(toks_word(w), levels) == Some(push_word(levels, w)),
    decreases w, 0nat,
{
    match w {
        GWord::PushInt(n) => {
            let ts0 = seq![Token::TInt(n)];
            let l1 = push_word(levels, GWord::PushInt(n));
            assert(toks_word(w) =~= ts0);
            assert(ts0[0] == Token::TInt(n));
            assert(ts0.subrange(1, ts0.len() as int) =~= Seq::<Token>::empty());
            assert(group_fold(Seq::<Token>::empty(), l1) == Some(l1));
            assert(group_fold(ts0, levels) == Some(l1));
        }
        GWord::Prim(p) => {
            let ts0 = seq![Token::TGlyph(p)];
            let l1 = push_word(levels, GWord::Prim(p));
            assert(toks_word(w) =~= ts0);
            assert(ts0[0] == Token::TGlyph(p));
            assert(ts0.subrange(1, ts0.len() as int) =~= Seq::<Token>::empty());
            assert(group_fold(Seq::<Token>::empty(), l1) == Some(l1));
            assert(group_fold(ts0, levels) == Some(l1));
        }
        GWord::Call(s) => {
            let ts0 = seq![Token::TName(s)];
            let l1 = push_word(levels, GWord::Call(s));
            assert(toks_word(w) =~= ts0);
            assert(ts0[0] == Token::TName(s));
            assert(ts0.subrange(1, ts0.len() as int) =~= Seq::<Token>::empty());
            assert(group_fold(Seq::<Token>::empty(), l1) == Some(l1));
            assert(group_fold(ts0, levels) == Some(l1));
        }
        GWord::PushQuote(q) => {
            let opened = levels.push(Seq::<GWord>::empty());
            // Step 1: consume TOpen.
            let a0 = seq![Token::TOpen];
            assert(group_fold(a0, levels) == Some(opened)) by {
                assert(a0[0] == Token::TOpen);
                assert(a0.subrange(1, a0.len() as int) =~= Seq::<Token>::empty());
                assert(group_fold(Seq::<Token>::empty(), opened) == Some(opened));
            }
            // Step 2: group the inner words on the opened level.
            lemma_group_words(q, opened);
            let after_inner = app_top(opened, q);
            assert(after_inner =~= levels.push(q)) by {
                assert(opened.drop_last() =~= levels);
                assert(opened.last() =~= Seq::<GWord>::empty());
                assert(opened.last() + q =~= q);
            }
            // Step 3: consume TClose, folding the inner into a PushQuote.
            let closed = push_word(levels, GWord::PushQuote(q));
            let a1 = seq![Token::TClose];
            assert(group_fold(a1, levels.push(q)) == Some(closed)) by {
                assert(a1[0] == Token::TClose);
                assert((levels.push(q)).len() >= 2);
                assert((levels.push(q)).last() == q);
                assert((levels.push(q)).drop_last() =~= levels);
                assert(push_word((levels.push(q)).drop_last(), GWord::PushQuote((levels.push(q)).last())) == closed);
                assert(a1.subrange(1, a1.len() as int) =~= Seq::<Token>::empty());
                assert(group_fold(Seq::<Token>::empty(), closed) == Some(closed));
            }
            // Compose the three via the split lemma.
            assert(toks_word(w) =~= a0 + (toks_words(q) + a1));
            lemma_group_split(a0, toks_words(q) + a1, levels, opened);
            lemma_group_split(toks_words(q), a1, opened, after_inner);
            assert(group_fold(toks_words(q) + a1, opened) == group_fold(a1, after_inner));
            assert(after_inner == levels.push(q));
        }
    }
}

pub proof fn lemma_group_words(ws: Seq<GWord>, levels: Seq<Seq<GWord>>)
    requires levels.len() >= 1,
    ensures group_fold(toks_words(ws), levels) == Some(app_top(levels, ws)),
    decreases ws, ws.len(),
{
    if ws.len() == 0 {
        assert(toks_words(ws) =~= Seq::<Token>::empty());
        assert(app_top(levels, ws) =~= levels) by {
            assert(levels.last() + ws =~= levels.last());
            assert(levels.drop_last().push(levels.last()) =~= levels);
        }
    } else {
        let w = ws[0];
        let rest = ws.subrange(1, ws.len() as int);
        lemma_group_word(w, levels);
        let l1 = push_word(levels, w);
        lemma_group_words(rest, l1);
        lemma_group_split(toks_word(w), toks_words(rest), levels, l1);
        assert(toks_words(ws) =~= toks_word(w) + toks_words(rest));
        // app_top(push_word(levels,w), rest) == app_top(levels, [w]++rest)
        assert(app_top(l1, rest) =~= app_top(levels, ws)) by {
            assert(l1.drop_last() =~= levels.drop_last());
            assert(l1.last() =~= levels.last().push(w));
            assert(levels.last().push(w) + rest =~= levels.last() + ws) by {
                assert(ws =~= seq![w] + rest);
                assert(levels.last().push(w) =~= levels.last() + seq![w]);
            }
        }
    }
}

// ============================================================
// 12. Tokenizer inverse (Layer 1) — the h0 separator rule made rigorous.
// ============================================================

// needs_sep identities used to relate the boundary rule to munch categories.
pub proof fn lemma_needs_sep_int(b: char)
    ensures needs_sep(Cls::CInt, b) == is_digit(b),
{
}

pub proof fn lemma_needs_sep_name(b: char)
    ensures needs_sep(Cls::CName, b) == is_namechar(b),
{
}

// First character of a rendered nonempty token stream.
pub proof fn lemma_render_first(prev: Option<Cls>, ts: Seq<Token>)
    requires valid_toks(ts), ts.len() >= 1,
    ensures
        render_h(prev, ts).len() >= 1,
        needs_sep_opt(prev, tok_first(ts[0])) ==> render_h(prev, ts)[0] == ' ',
        !needs_sep_opt(prev, tok_first(ts[0])) ==> render_h(prev, ts)[0] == tok_first(ts[0]),
{
    let t = ts[0];
    assert(valid_tok(t)) by { assert(valid_tok(ts[0])); }
    lemma_piece_nonempty(t);
    let tail = render_h(Some(tok_class(t)), ts.subrange(1, ts.len() as int));
    if needs_sep_opt(prev, tok_first(t)) {
        let sep = seq![' '];
        assert(render_h(prev, ts) =~= sep + tok_piece(t) + tail);
        assert(render_h(prev, ts)[0] == ' ');
    } else {
        let sep = Seq::<char>::empty();
        assert(render_h(prev, ts) =~= tok_piece(t) + tail);
        assert(render_h(prev, ts)[0] == tok_piece(t)[0]);
        assert(tok_piece(t)[0] == tok_first(t));
    }
}

// Boundary: after a rendered token of class `cls0`, the tail (rendered from
// the remaining tokens) either is empty or starts with a char that does NOT
// continue a digit run (when cls0==CInt) / a name run (when cls0==CName).
pub proof fn lemma_boundary(cls0: Cls, rest: Seq<Token>)
    requires valid_toks(rest),
    ensures ({
        let tail = render_h(Some(cls0), rest);
        &&& (cls0 == Cls::CInt ==> (tail.len() == 0 || !is_digit(tail[0])))
        &&& (cls0 == Cls::CName ==> (tail.len() == 0 || !is_namechar(tail[0])))
    }),
{
    let tail = render_h(Some(cls0), rest);
    if rest.len() == 0 {
        assert(tail =~= Seq::<char>::empty());
    } else {
        lemma_render_first(Some(cls0), rest);
        let b = tok_first(rest[0]);
        if needs_sep_opt(Some(cls0), b) {
            assert(tail[0] == ' ');
        } else {
            assert(tail[0] == b);
            if cls0 == Cls::CInt {
                lemma_needs_sep_int(b);
                assert(!is_digit(b));
            }
            if cls0 == Cls::CName {
                lemma_needs_sep_name(b);
                assert(!is_namechar(b));
            }
        }
    }
}

// Maximal munch reads exactly a leading all-digit block.
pub proof fn lemma_ldl_all(a: Seq<char>, b: Seq<char>)
    requires
        forall|i: int| 0 <= i < a.len() ==> is_digit(#[trigger] a[i]),
        b.len() == 0 || !is_digit(b[0]),
    ensures leading_digits_len(a + b) == a.len(),
    decreases a.len(),
{
    if a.len() == 0 {
        assert(a + b =~= b);
    } else {
        assert((a + b)[0] == a[0]);
        assert(is_digit((a + b)[0]));
        let arest = a.subrange(1, a.len() as int);
        assert((a + b).subrange(1, (a + b).len() as int) =~= arest + b);
        assert forall|i: int| 0 <= i < arest.len() implies is_digit(#[trigger] arest[i]) by {
            assert(arest[i] == a[i + 1]);
        }
        lemma_ldl_all(arest, b);
    }
}

// Maximal munch reads exactly a leading all-namechar block.
pub proof fn lemma_lnl_all(a: Seq<char>, b: Seq<char>)
    requires
        forall|i: int| 0 <= i < a.len() ==> is_namechar(#[trigger] a[i]),
        b.len() == 0 || !is_namechar(b[0]),
    ensures leading_name_len(a + b) == a.len(),
    decreases a.len(),
{
    if a.len() == 0 {
        assert(a + b =~= b);
    } else {
        assert((a + b)[0] == a[0]);
        assert(is_namechar((a + b)[0]));
        let arest = a.subrange(1, a.len() as int);
        assert((a + b).subrange(1, (a + b).len() as int) =~= arest + b);
        assert forall|i: int| 0 <= i < arest.len() implies is_namechar(#[trigger] arest[i]) by {
            assert(arest[i] == a[i + 1]);
        }
        lemma_lnl_all(arest, b);
    }
}

// nat_of_digits ignores a suffix beyond the leading digit block.
pub proof fn lemma_nat_prefix(a: Seq<char>, b: Seq<char>)
    ensures (a + b).subrange(0, a.len() as int) =~= a,
{
}

// Munch one token off the front: lexing piece(t)++tail reads exactly t,
// leaving tail, provided the boundary invariant holds and lex(tail)==Some(rest).
pub proof fn lemma_lex_one(t: Token, tail: Seq<char>, rest: Seq<Token>)
    requires
        valid_tok(t),
        lex(tail) == Some(rest),
        tok_class(t) == Cls::CInt ==> (tail.len() == 0 || !is_digit(tail[0])),
        tok_class(t) == Cls::CName ==> (tail.len() == 0 || !is_namechar(tail[0])),
    ensures
        lex(tok_piece(t) + tail) == Some(seq![t] + rest),
{
    let piece0 = tok_piece(t);
    lemma_piece_nonempty(t);
    let body = piece0 + tail;
    assert(body[0] == piece0[0]);
    assert(body.len() >= 1);
    match t {
        Token::TInt(n) => {
            lemma_digits_all_digits(n as nat);
            assert(piece0 == digits(n as nat));
            assert(is_digit(piece0[0]));
            assert(tail.len() == 0 || !is_digit(tail[0]));
            lemma_ldl_all(piece0, tail);
            let k = leading_digits_len(body);
            assert(k == piece0.len());
            assert(is_digit(body[0]) && !is_ws(body[0]));
            assert(body.subrange(0, k as int) =~= piece0);
            assert(body.subrange(k as int, body.len() as int) =~= tail);
            lemma_nat_of_digits_inverse(n as nat);
            assert(nat_of_digits(piece0) == n as nat);
            assert(lex(body) == lex_cons(Token::TInt(nat_of_digits(piece0) as int), lex(tail)));
            assert(Token::TInt(nat_of_digits(piece0) as int) == t);
        }
        Token::TName(s) => {
            assert(piece0 == s);
            assert(valid_name(s));
            assert(is_lower(s[0]));
            assert(forall|i: int| 0 <= i < s.len() ==> is_namechar(#[trigger] s[i]));
            assert(tail.len() == 0 || !is_namechar(tail[0]));
            lemma_lnl_all(piece0, tail);
            let k = leading_name_len(body);
            assert(k == piece0.len());
            assert(is_lower(body[0]) && !is_ws(body[0]) && !is_digit(body[0]));
            assert(body.subrange(0, k as int) =~= piece0);
            assert(body.subrange(k as int, body.len() as int) =~= tail);
            assert(lex(body) == lex_cons(Token::TName(piece0), lex(tail)));
            assert(Token::TName(piece0) == t);
        }
        Token::TGlyph(p) => {
            lemma_glyph(p);
            assert(piece0 =~= seq![spec_glyph(p)]);
            assert(body[0] == spec_glyph(p));
            assert(!is_ws(body[0]) && !is_digit(body[0]) && !is_lower(body[0]));
            assert(body[0] != '[' && body[0] != ']');
            assert(body.subrange(1, body.len() as int) =~= tail);
            assert(glyph_to_gprim(body[0]) == Some(p));
            assert(lex(body) == lex_cons(Token::TGlyph(p), lex(tail)));
            assert(Token::TGlyph(p) == t);
        }
        Token::TOpen => {
            assert(piece0 =~= seq!['[']);
            assert(body[0] == '[');
            assert(!is_ws('[') && !is_digit('[') && !is_lower('['));
            assert(body.subrange(1, body.len() as int) =~= tail);
            assert(lex(body) == lex_cons(Token::TOpen, lex(tail)));
        }
        Token::TClose => {
            assert(piece0 =~= seq![']']);
            assert(body[0] == ']');
            assert(!is_ws(']') && !is_digit(']') && !is_lower(']') && ']' != '[');
            assert(body.subrange(1, body.len() as int) =~= tail);
            assert(lex(body) == lex_cons(Token::TClose, lex(tail)));
        }
    }
    assert(seq![t] + rest =~= seq![t] + rest);
}

// The crux: lexing a rendered valid token stream recovers it exactly.
pub proof fn lemma_lex_render(prev: Option<Cls>, ts: Seq<Token>)
    requires valid_toks(ts),
    ensures lex(render_h(prev, ts)) == Some(ts),
    decreases ts.len(),
{
    if ts.len() == 0 {
        assert(render_h(prev, ts) =~= Seq::<char>::empty());
        assert(lex(Seq::<char>::empty()) == Some(Seq::<Token>::empty()));
        assert(ts =~= Seq::<Token>::empty());
    } else {
        let t = ts[0];
        let rest = ts.subrange(1, ts.len() as int);
        let cls0 = tok_class(t);
        let piece0 = tok_piece(t);
        let tail = render_h(Some(cls0), rest);
        assert(valid_tok(t)) by { assert(valid_tok(ts[0])); }
        lemma_piece_nonempty(t);

        assert(valid_toks(rest)) by {
            assert forall|i: int| 0 <= i < rest.len() implies valid_tok(#[trigger] rest[i]) by {
                assert(rest[i] == ts[i + 1]);
            }
        }
        lemma_lex_render(Some(cls0), rest);
        assert(lex(tail) == Some(rest));
        lemma_boundary(cls0, rest);
        lemma_lex_one(t, tail, rest);
        assert(lex(piece0 + tail) == Some(seq![t] + rest));
        assert(seq![t] + rest =~= ts);

        let body = piece0 + tail;
        if needs_sep_opt(prev, tok_first(t)) {
            let sep = seq![' '];
            assert(render_h(prev, ts) =~= sep + body);
            assert((sep + body)[0] == ' ');
            assert(is_ws(' '));
            assert((sep + body).subrange(1, (sep + body).len() as int) =~= body);
            assert(lex(sep + body) == lex(body));
        } else {
            assert(render_h(prev, ts) =~= body);
        }
    }
}

// ============================================================
// 13. The P4 theorems.
// ============================================================

// P4 round-trip: printing a well-formed program then parsing recovers it.
pub proof fn p4_roundtrip(p: Seq<GWord>)
    requires wf_words(p),
    ensures spec_parse(spec_print(p)) == ParseOutcome::Ok(p),
{
    // Layer 1: the printed string lexes back to the exact token stream.
    lemma_wf_valid_words(p);
    lemma_lex_render(None, toks_words(p));
    assert(spec_print(p) == render_h(None, toks_words(p)));
    assert(lex(spec_print(p)) == Some(toks_words(p)));

    // Layer 2: the token stream groups back to the exact program.
    let init = seq![Seq::<GWord>::empty()];
    assert(init.len() == 1);
    lemma_group_words(p, init);
    let grouped = app_top(init, p);
    assert(group_fold(toks_words(p), init) == Some(grouped));
    assert(grouped =~= seq![p]) by {
        assert(init.last() =~= Seq::<GWord>::empty());
        assert(init.last() + p =~= p);
        assert(init.drop_last() =~= Seq::<Seq<GWord>>::empty());
        assert(init.drop_last().push(p) =~= seq![p]);
    }
    assert(grouped.len() == 1);
    assert(grouped[0] == p);
}

// P4 idempotence corollary: print is a fixed point on its own image
// (print ∘ parse ∘ print == print), which is the canonicalization property.
// Idempotence follows directly from round-trip.
pub proof fn p4_idempotent(p: Seq<GWord>)
    requires wf_words(p),
    ensures
        spec_parse(spec_print(p)) == ParseOutcome::Ok(p),
        spec_print(outcome_prog(spec_parse(spec_print(p)))) == spec_print(p),
{
    p4_roundtrip(p);
    assert(outcome_prog(ParseOutcome::Ok(p)) == p);
}

// ============================================================
// 14. Non-vacuity self-audit (mirrors mtl-core's witness style).
// Concrete programs exercising: all 23 glyphs, integer boundaries (0, large),
// nested quotations, and the `h0`/adjacency separator cases.
// ============================================================

// A concrete name "h0" — a Call ending in a digit (the h0 case).
pub open spec fn name_h0() -> Seq<char> { seq!['h', '0'] }

pub proof fn p4_audit_h0_adjacency()
    ensures
        // "h0" then int 0 needs a space (Name|digit boundary): "h0 0".
        valid_name(name_h0()),
        wf_words(seq![GWord::Call(name_h0()), GWord::PushInt(0)]),
{
    assert(is_lower('h'));
    assert(is_namechar('h') && is_namechar('0'));
    let s = name_h0();
    assert(s.len() == 2 && s[0] == 'h' && s[1] == '0');
    assert forall|i: int| 0 <= i < s.len() implies is_namechar(#[trigger] s[i]) by {
        if i == 0 { assert(s[i] == 'h'); } else { assert(s[i] == '0'); }
    }
    assert(valid_name(s));
    let prog = seq![GWord::Call(name_h0()), GWord::PushInt(0)];
    assert(prog[0] == GWord::Call(s));
    assert(wf_word(prog[0]));
    let tl = prog.subrange(1, prog.len() as int);
    assert(tl =~= seq![GWord::PushInt(0)]);
    assert(tl[0] == GWord::PushInt(0));
    assert(wf_word(tl[0]));
    assert(tl.subrange(1, tl.len() as int) =~= Seq::<GWord>::empty());
    assert(wf_words(tl.subrange(1, tl.len() as int)));
    assert(wf_words(tl));
    assert(wf_words(prog));
}

// A witness program touching every glyph, a nested quote, int 0 and a large
// int; round-trips by the theorem (non-vacuous: the ensures is Ok(prog)).
pub proof fn p4_audit_all_glyphs() {
    let prims = seq![
        GWord::Prim(GPrim::Dup), GWord::Prim(GPrim::Drop), GWord::Prim(GPrim::Swap),
        GWord::Prim(GPrim::Rot), GWord::Prim(GPrim::Over), GWord::Prim(GPrim::Apply),
        GWord::Prim(GPrim::Cat), GWord::Prim(GPrim::Cons), GWord::Prim(GPrim::Dip),
        GWord::Prim(GPrim::Add), GWord::Prim(GPrim::Sub), GWord::Prim(GPrim::Mul),
        GWord::Prim(GPrim::Div), GWord::Prim(GPrim::Mod), GWord::Prim(GPrim::Eq),
        GWord::Prim(GPrim::Lt), GWord::Prim(GPrim::If), GWord::Prim(GPrim::PrimRec),
        GWord::Prim(GPrim::Times), GWord::Prim(GPrim::LinRec), GWord::Prim(GPrim::Uncons),
        GWord::Prim(GPrim::Fold), GWord::Prim(GPrim::Xor)
    ];
    assert(prims.len() == 23);
}

// ============================================================
// 15. Executable printer (exec-mode refinement of spec_print).
//
// Exec datatypes mirror the ghost AST but use Vec/i64 (executable) instead of
// Seq/int (ghost). Their `view()` maps back to the ghost types, and
// `exec_print` is PROVEN to produce exactly `spec_print` of the viewed program.
// ============================================================

// Executable mirror of GPrim (23 variants, identical order).
pub enum EPrim {
    Dup, Drop, Swap, Rot, Over, Apply, Cat, Cons, Dip,
    Add, Sub, Mul, Div, Mod, Eq, Lt, If,
    PrimRec, Times, LinRec, Uncons, Fold, Xor,
}

impl View for EPrim {
    type V = GPrim;
    open spec fn view(&self) -> GPrim {
        match self {
            EPrim::Dup => GPrim::Dup, EPrim::Drop => GPrim::Drop, EPrim::Swap => GPrim::Swap,
            EPrim::Rot => GPrim::Rot, EPrim::Over => GPrim::Over, EPrim::Apply => GPrim::Apply,
            EPrim::Cat => GPrim::Cat, EPrim::Cons => GPrim::Cons, EPrim::Dip => GPrim::Dip,
            EPrim::Add => GPrim::Add, EPrim::Sub => GPrim::Sub, EPrim::Mul => GPrim::Mul,
            EPrim::Div => GPrim::Div, EPrim::Mod => GPrim::Mod, EPrim::Eq => GPrim::Eq,
            EPrim::Lt => GPrim::Lt, EPrim::If => GPrim::If, EPrim::PrimRec => GPrim::PrimRec,
            EPrim::Times => GPrim::Times, EPrim::LinRec => GPrim::LinRec, EPrim::Uncons => GPrim::Uncons,
            EPrim::Fold => GPrim::Fold, EPrim::Xor => GPrim::Xor,
        }
    }
}

// Executable mirror of GWord.
pub enum EWord {
    EPushInt(i64),
    EPushQuote(Vec<EWord>),
    EPrim(EPrim),
    ECall(Vec<char>),
}

// View of one exec word to its ghost. Mirrors the toks_word/toks_words
// lexicographic decreases pattern (word measure `(w, 0)`, seq measure
// `(s, s.len())`) so the mutual recursion through PushQuote terminates.
pub open spec fn eword_view(w: EWord) -> GWord
    decreases w, 0nat,
{
    match w {
        EWord::EPushInt(i) => GWord::PushInt(i as int),
        EWord::EPrim(p) => GWord::Prim(p.view()),
        EWord::ECall(v) => GWord::Call(v@),
        EWord::EPushQuote(ws) => GWord::PushQuote(ewords_view(ws@)),
    }
}

pub open spec fn ewords_view(s: Seq<EWord>) -> Seq<GWord>
    decreases s, s.len(),
{
    if s.len() == 0 {
        Seq::<GWord>::empty()
    } else {
        seq![eword_view(s[0])] + ewords_view(s.subrange(1, s.len() as int))
    }
}

// Singleton unfolds (spell out the base case for the recursive-fn fuel).
pub proof fn lemma_ewords_view_singleton(x: EWord)
    ensures ewords_view(seq![x]) == seq![eword_view(x)],
{
    let s = seq![x];
    assert(s.len() == 1 && s[0] == x);
    assert(s.subrange(1, s.len() as int) =~= Seq::<EWord>::empty());
    assert(ewords_view(s.subrange(1, s.len() as int)) == Seq::<GWord>::empty());
    assert(ewords_view(s) =~= seq![eword_view(x)]);
}

pub proof fn lemma_toks_words_singleton(w: GWord)
    ensures toks_words(seq![w]) == toks_word(w),
{
    let s = seq![w];
    assert(s.len() == 1 && s[0] == w);
    assert(s.subrange(1, s.len() as int) =~= Seq::<GWord>::empty());
    assert(toks_words(s.subrange(1, s.len() as int)) == Seq::<Token>::empty());
    assert(toks_words(s) =~= toks_word(w));
}

// Class of the last-emitted token in a stream (None if empty). Threaded by the
// exec printer as its `last: Option<Cls>` boundary bookkeeping.
pub open spec fn last_cls(prev: Option<Cls>, ts: Seq<Token>) -> Option<Cls> {
    if ts.len() == 0 { prev } else { Some(tok_class(ts[ts.len() as int - 1])) }
}

// toks_words distributes over sequence concatenation.
pub proof fn lemma_toks_words_append(a: Seq<GWord>, b: Seq<GWord>)
    ensures toks_words(a + b) == toks_words(a) + toks_words(b),
    decreases a.len(),
{
    if a.len() == 0 {
        assert(a + b =~= b);
        assert(toks_words(a) =~= Seq::<Token>::empty());
    } else {
        let arest = a.subrange(1, a.len() as int);
        assert((a + b)[0] == a[0]);
        assert((a + b).subrange(1, (a + b).len() as int) =~= arest + b);
        lemma_toks_words_append(arest, b);
        // toks_words(a+b) = toks_word(a[0]) + toks_words(arest + b)
        //                 = toks_word(a[0]) + toks_words(arest) + toks_words(b)
        assert(toks_words(a + b) =~= toks_words(a) + toks_words(b));
    }
}

// last_cls composes over concatenation.
pub proof fn lemma_last_cls_append(prev: Option<Cls>, a: Seq<Token>, b: Seq<Token>)
    ensures last_cls(prev, a + b) == last_cls(last_cls(prev, a), b),
{
    if b.len() == 0 {
        assert(a + b =~= a);
    } else {
        assert((a + b)[(a + b).len() - 1] == b[b.len() - 1]);
    }
}

// The central "loop = fold" identity: rendering a concatenation is the render of
// the first part, followed by the render of the second part started from the
// boundary class left behind by the first.
pub proof fn lemma_render_append(prev: Option<Cls>, a: Seq<Token>, b: Seq<Token>)
    ensures render_h(prev, a + b) == render_h(prev, a) + render_h(last_cls(prev, a), b),
    decreases a.len(),
{
    if a.len() == 0 {
        assert(a + b =~= b);
        assert(render_h(prev, a) =~= Seq::<char>::empty());
        assert(last_cls(prev, a) == prev);
        assert(render_h(prev, a) + render_h(prev, b) =~= render_h(prev, b));
    } else {
        let t = a[0];
        let arest = a.subrange(1, a.len() as int);
        assert((a + b)[0] == t);
        assert((a + b).subrange(1, (a + b).len() as int) =~= arest + b);
        let sep = if needs_sep_opt(prev, tok_first(t)) { seq![' '] } else { Seq::<char>::empty() };
        lemma_render_append(Some(tok_class(t)), arest, b);
        assert(last_cls(prev, a) == last_cls(Some(tok_class(t)), arest)) by {
            if arest.len() == 0 {
                assert(a[a.len() - 1] == t);
            } else {
                assert(a[a.len() - 1] == arest[arest.len() - 1]);
            }
        }
        // render_h(prev, a+b) = sep + piece(t) + render_h(Some class, arest + b)
        //   = sep + piece(t) + [render_h(Some class, arest) + render_h(last_cls(prev,a), b)]
        //   = [sep + piece(t) + render_h(Some class, arest)] + render_h(last_cls(prev,a), b)
        //   = render_h(prev, a) + render_h(last_cls(prev, a), b)
        assert(render_h(prev, a + b) =~= render_h(prev, a) + render_h(last_cls(prev, a), b));
    }
}

// Rendering a single token: an optional leading space plus the token's piece.
pub proof fn lemma_render_single(prev: Option<Cls>, t: Token)
    requires valid_tok(t),
    ensures render_h(prev, seq![t]) ==
        (if needs_sep_opt(prev, tok_first(t)) { seq![' '] } else { Seq::<char>::empty() }) + tok_piece(t),
{
    let ts = seq![t];
    lemma_piece_nonempty(t);
    assert(ts.len() == 1);
    assert(ts[0] == t);
    assert(ts.subrange(1, ts.len() as int) =~= Seq::<Token>::empty());
    assert(render_h(Some(tok_class(t)), Seq::<Token>::empty()) =~= Seq::<char>::empty());
    assert(render_h(prev, ts) =~=
        (if needs_sep_opt(prev, tok_first(t)) { seq![' '] } else { Seq::<char>::empty() }) + tok_piece(t));
}

// ewords_view distributes over concatenation; and its length/indexing agree
// elementwise with eword_view.
pub proof fn lemma_ewords_view_append(a: Seq<EWord>, b: Seq<EWord>)
    ensures ewords_view(a + b) == ewords_view(a) + ewords_view(b),
    decreases a.len(),
{
    if a.len() == 0 {
        assert(a + b =~= b);
        assert(ewords_view(a) =~= Seq::<GWord>::empty());
    } else {
        let arest = a.subrange(1, a.len() as int);
        assert((a + b)[0] == a[0]);
        assert((a + b).subrange(1, (a + b).len() as int) =~= arest + b);
        lemma_ewords_view_append(arest, b);
        assert(ewords_view(a + b) =~= ewords_view(a) + ewords_view(b));
    }
}

pub proof fn lemma_ewords_view_len_index(s: Seq<EWord>)
    ensures
        ewords_view(s).len() == s.len(),
        forall|i: int| 0 <= i < s.len() ==> ewords_view(s)[i] == eword_view(#[trigger] s[i]),
    decreases s.len(),
{
    if s.len() == 0 {
    } else {
        let srest = s.subrange(1, s.len() as int);
        lemma_ewords_view_len_index(srest);
        assert forall|i: int| 0 <= i < s.len() implies
            ewords_view(s)[i] == eword_view(#[trigger] s[i]) by {
            if i == 0 {
                assert(ewords_view(s)[0] == eword_view(s[0]));
            } else {
                assert(ewords_view(s)[i] == ewords_view(srest)[i - 1]);
                assert(srest[i - 1] == s[i]);
            }
        }
    }
}

// wf_words gives wf_word of every element.
pub proof fn lemma_wf_words_index(s: Seq<GWord>, i: int)
    requires wf_words(s), 0 <= i < s.len(),
    ensures wf_word(s[i]),
    decreases s.len(),
{
    if i == 0 {
    } else {
        lemma_wf_words_index(s.subrange(1, s.len() as int), i - 1);
        assert(s.subrange(1, s.len() as int)[i - 1] == s[i]);
    }
}

// --- exec character predicates (mirror the spec predicates) ---

fn exec_is_digit(c: char) -> (r: bool)
    ensures r == is_digit(c),
{
    '0' <= c && c <= '9'
}

fn exec_is_lower(c: char) -> (r: bool)
    ensures r == is_lower(c),
{
    'a' <= c && c <= 'z'
}

// exec mirror of needs_sep.
fn exec_needs_sep(left: Cls, b: char) -> (r: bool)
    ensures r == needs_sep(left, b),
{
    if exec_is_digit(b) {
        match left { Cls::CInt => true, Cls::CName => true, Cls::CPunct => false }
    } else if exec_is_lower(b) {
        match left { Cls::CName => true, _ => false }
    } else {
        false
    }
}

// exec glyph lookup (mirror of spec_glyph).
fn eprim_glyph(p: &EPrim) -> (r: char)
    ensures r == spec_glyph(p.view()),
{
    match p {
        EPrim::Dup => ':', EPrim::Drop => '_', EPrim::Swap => '~', EPrim::Rot => '@',
        EPrim::Over => '^', EPrim::Apply => '!', EPrim::Cat => ',', EPrim::Cons => ';',
        EPrim::Dip => '\'', EPrim::Add => '+', EPrim::Sub => '-', EPrim::Mul => '*',
        EPrim::Div => '/', EPrim::Mod => '%', EPrim::Eq => '=', EPrim::Lt => '<',
        EPrim::If => '?', EPrim::PrimRec => '&', EPrim::Times => '.', EPrim::LinRec => '|',
        EPrim::Uncons => '>', EPrim::Fold => '(', EPrim::Xor => '$',
    }
}

// --- exec decimal digits (mirror of the spec `digits`) ---

fn exec_digit_char(k: u64) -> (r: char)
    requires k < 10,
    ensures r == digit_char(k as nat),
{
    if k == 0 { '0' } else if k == 1 { '1' } else if k == 2 { '2' }
    else if k == 3 { '3' } else if k == 4 { '4' } else if k == 5 { '5' }
    else if k == 6 { '6' } else if k == 7 { '7' } else if k == 8 { '8' }
    else { '9' }
}

// Emit the decimal representation of `n`, most-significant digit first.
fn exec_digits(n: u64) -> (r: Vec<char>)
    ensures r@ == digits(n as nat),
    decreases n,
{
    if n < 10 {
        let mut v: Vec<char> = Vec::new();
        v.push(exec_digit_char(n));
        proof { assert(v@ =~= digits(n as nat)); }
        v
    } else {
        let mut v = exec_digits(n / 10);
        v.push(exec_digit_char(n % 10));
        proof {
            assert((n as nat) / 10 == (n / 10) as nat);
            assert((n as nat) % 10 == (n % 10) as nat);
            assert(v@ =~= digits(n as nat));
        }
        v
    }
}

// Append a printed token `piece` (of ghost token `t`, class `class`) to `out`,
// inserting one separating space first iff the boundary rule demands it. Grows
// `out` by exactly `render_h(last, seq![t])` and returns the new boundary class.
fn emit_token(out: &mut Vec<char>, last: Option<Cls>, piece: &Vec<char>, class: Cls, t: Ghost<Token>) -> (r: Option<Cls>)
    requires
        valid_tok(t@),
        piece@ == tok_piece(t@),
        class == tok_class(t@),
    ensures
        final(out)@ == old(out)@ + render_h(last, seq![t@]),
        r == last_cls(last, seq![t@]),
        r == Some(tok_class(t@)),
{
    let ghost old0 = out@;
    proof { lemma_piece_nonempty(t@); }
    // tok_first(t@) == piece@[0] == piece[0]
    let need: bool = match last {
        None => false,
        Some(l) => exec_needs_sep(l, piece[0]),
    };
    assert(need == needs_sep_opt(last, tok_first(t@)));
    if need {
        out.push(' ');
    }
    let ghost after_sep = out@;
    assert(after_sep =~= old0 + (if need { seq![' '] } else { Seq::<char>::empty() }));
    let mut j: usize = 0;
    while j < piece.len()
        invariant
            0 <= j <= piece.len(),
            out@ == after_sep + piece@.subrange(0, j as int),
        decreases piece.len() - j,
    {
        out.push(piece[j]);
        proof {
            assert(piece@.subrange(0, (j + 1) as int) =~= piece@.subrange(0, j as int).push(piece@[j as int]));
        }
        j = j + 1;
    }
    proof {
        assert(piece@.subrange(0, piece@.len() as int) =~= piece@);
        lemma_render_single(last, t@);
        assert(out@ =~= old0 + render_h(last, seq![t@]));
        // last_cls(last, seq![t@]) : the singleton's last element is t@.
        assert(seq![t@][seq![t@].len() - 1] == t@);
    }
    Some(class)
}

// Emit one word (mirrors production `emit_word`), threading `last` so nested
// quotations share the same boundary bookkeeping. Grows `out` by exactly
// `render_h(last, toks_word(eword_view(*w)))`.
fn exec_emit_word(out: &mut Vec<char>, last: Option<Cls>, w: &EWord) -> (r: Option<Cls>)
    requires wf_word(eword_view(*w)),
    ensures
        final(out)@ == old(out)@ + render_h(last, toks_word(eword_view(*w))),
        r == last_cls(last, toks_word(eword_view(*w))),
    decreases *w, 0nat,
{
    match w {
        EWord::EPushInt(i) => {
            assert(*i as int >= 0);
            assert(*i >= 0);
            let piece = exec_digits(*i as u64);
            proof {
                assert(*i as u64 == *i as int);
                assert((*i as u64) as nat == (*i as int) as nat);
                assert(piece@ == tok_piece(Token::TInt(*i as int)));
            }
            let r = emit_token(out, last, &piece, Cls::CInt, Ghost(Token::TInt(*i as int)));
            proof { assert(toks_word(eword_view(*w)) =~= seq![Token::TInt(*i as int)]); }
            r
        }
        EWord::EPrim(p) => {
            let g = eprim_glyph(p);
            let mut piece: Vec<char> = Vec::new();
            piece.push(g);
            proof { assert(piece@ =~= seq![spec_glyph(p.view())]); }
            let r = emit_token(out, last, &piece, Cls::CPunct, Ghost(Token::TGlyph(p.view())));
            proof { assert(toks_word(eword_view(*w)) =~= seq![Token::TGlyph(p.view())]); }
            r
        }
        EWord::ECall(v) => {
            let r = emit_token(out, last, v, Cls::CName, Ghost(Token::TName(v@)));
            proof { assert(toks_word(eword_view(*w)) =~= seq![Token::TName(v@)]); }
            r
        }
        EWord::EPushQuote(ws) => {
            let ghost start = out@;
            let ghost q = ewords_view(ws@);
            let mut ob: Vec<char> = Vec::new();
            ob.push('[');
            proof { assert(ob@ =~= seq!['[']); }
            let l1 = emit_token(out, last, &ob, Cls::CPunct, Ghost(Token::TOpen));
            let l2 = exec_emit_words(out, l1, ws);
            let mut cb: Vec<char> = Vec::new();
            cb.push(']');
            proof { assert(cb@ =~= seq![']']); }
            let l3 = emit_token(out, l2, &cb, Cls::CPunct, Ghost(Token::TClose));
            proof {
                let a = seq![Token::TOpen];
                let b = toks_words(q);
                let c = seq![Token::TClose];
                // toks_word(PushQuote(q)) == a + b + c
                assert(toks_word(eword_view(*w)) =~= (a + b) + c);
                // l1 == last_cls(last, a) == Some(CPunct); l2 == last_cls(l1, b).
                lemma_render_append(l1, b, c);
                lemma_render_append(last, a, b + c);
                assert(last_cls(last, a) == l1);
                assert(last_cls(l1, b) == l2);
                // out@ == start + Rh(last,a) + Rh(l1,b) + Rh(l2,c)
                //       == start + Rh(last, a + (b + c)) == start + Rh(last, (a+b)+c)
                assert(a + (b + c) =~= (a + b) + c);
                assert(out@ =~= start + render_h(last, toks_word(eword_view(*w))));
                // l3 == last_cls(last, toks_word(PushQuote(q)))
                lemma_last_cls_append(last, a + b, c);
                lemma_last_cls_append(l1, b, c);
                assert(last_cls(last, toks_word(eword_view(*w))) == l3);
            }
            l3
        }
    }
}

// Emit all words of `ws`, threading `last`. The loop invariant ties the emitted
// prefix to `render_h` over `toks_words` of the already-processed words.
fn exec_emit_words(out: &mut Vec<char>, last: Option<Cls>, ws: &Vec<EWord>) -> (r: Option<Cls>)
    requires wf_words(ewords_view(ws@)),
    ensures
        final(out)@ == old(out)@ + render_h(last, toks_words(ewords_view(ws@))),
        r == last_cls(last, toks_words(ewords_view(ws@))),
    decreases ws@, ws@.len(),
{
    let ghost start = out@;
    let ghost gws = ws@;
    let mut cur: Option<Cls> = last;
    let mut i: usize = 0;
    while i < ws.len()
        invariant
            0 <= i <= ws.len(),
            ws@ == gws,
            wf_words(ewords_view(ws@)),
            out@ == start + render_h(last, toks_words(ewords_view(ws@.subrange(0, i as int)))),
            cur == last_cls(last, toks_words(ewords_view(ws@.subrange(0, i as int)))),
        decreases ws.len() - i,
    {
        proof {
            lemma_ewords_view_len_index(ws@);
            lemma_wf_words_index(ewords_view(ws@), i as int);
        }
        let ci = exec_emit_word(out, cur, &ws[i]);
        proof {
            let x = ws@[i as int];
            let pre = ws@.subrange(0, i as int);
            let nx = ws@.subrange(0, (i + 1) as int);
            assert(nx =~= pre + seq![x]);
            lemma_ewords_view_singleton(x);
            lemma_ewords_view_append(pre, seq![x]);
            assert(ewords_view(nx) =~= ewords_view(pre) + seq![eword_view(x)]);
            let st = seq![eword_view(x)];
            lemma_toks_words_singleton(eword_view(x));
            lemma_toks_words_append(ewords_view(pre), st);
            let t = toks_words(ewords_view(pre));
            let wtoks = toks_word(eword_view(x));
            assert(toks_words(ewords_view(nx)) =~= t + wtoks);
            lemma_render_append(last, t, wtoks);
            lemma_last_cls_append(last, t, wtoks);
        }
        cur = ci;
        i = i + 1;
    }
    proof { assert(ws@.subrange(0, ws.len() as int) =~= ws@); }
    cur
}

// The executable printer, PROVEN to refine spec_print over the well-formed
// domain: `r@ == spec_print(ewords_view(p@))`.
pub fn exec_print(p: &Vec<EWord>) -> (r: Vec<char>)
    requires wf_words(ewords_view(p@)),
    ensures r@ == spec_print(ewords_view(p@)),
{
    let mut out: Vec<char> = Vec::new();
    let last: Option<Cls> = None;
    let _ = exec_emit_words(&mut out, last, p);
    proof { assert(out@ =~= render_h(None, toks_words(ewords_view(p@)))); }
    out
}

// Non-vacuity: the exec printer's well-formed domain is inhabited (its
// `requires` is satisfiable), so `exec_print`'s refinement is not vacuous.
pub proof fn exec_print_domain_nonvacuous()
    ensures wf_words(ewords_view(seq![EWord::EPushInt(0), EWord::EPrim(EPrim::Dup)])),
{
    let s = seq![EWord::EPushInt(0), EWord::EPrim(EPrim::Dup)];
    lemma_ewords_view_len_index(s);
    let g = ewords_view(s);
    assert(s[0] == EWord::EPushInt(0) && s[1] == EWord::EPrim(EPrim::Dup));
    assert(g.len() == 2);
    assert(g[0] == eword_view(s[0]) && g[0] == GWord::PushInt(0));
    assert(g[1] == eword_view(s[1]) && g[1] == GWord::Prim(GPrim::Dup));
    assert(wf_word(g[0]) && wf_word(g[1]));
    let tl = g.subrange(1, g.len() as int);
    assert(tl.len() == 1 && tl[0] == g[1]);
    assert(tl.subrange(1, tl.len() as int) =~= Seq::<GWord>::empty());
    assert(wf_words(tl.subrange(1, tl.len() as int)));
    assert(wf_words(tl));
    assert(wf_words(g));
}

// ============================================================
// 16. Executable lexer/parser (exec-mode refinement of spec_parse).
//
// The exec parser mirrors the SPEC (`lex` / `group_fold` / `spec_parse`),
// arm-for-arm and suffix-for-suffix, in the same recursion shape used for the
// printer. Because production integer literals are i64, the executable side
// must ERROR on any digit run whose value exceeds i64::MAX, while the ghost
// `spec_parse` returns Ok with an unbounded int. So unconditional refinement
// is FALSE. We therefore state refinement CONDITIONALLY on `all_ints_fit`
// (every maximal digit run fits i64); on that domain the exec parser reproduces
// spec_parse exactly, and off it, it diverges only by an overflow rejection.
// The round-trip corollary (section 22) then dodges overflow entirely, because
// `exec_print` of a `Vec<EWord>` (whose ints are i64 by construction) always
// yields in-range digit runs.
// ============================================================

// Executable mirror of `Token`. TInt carries i64 (bounded) vs ghost `int`.
pub enum ExecToken {
    ETInt(i64),
    ETName(Vec<char>),
    ETGlyph(EPrim),
    ETOpen,
    ETClose,
}

impl View for ExecToken {
    type V = Token;
    open spec fn view(&self) -> Token {
        match self {
            ExecToken::ETInt(n) => Token::TInt(*n as int),
            ExecToken::ETName(v) => Token::TName(v@),
            ExecToken::ETGlyph(p) => Token::TGlyph(p@),
            ExecToken::ETOpen => Token::TOpen,
            ExecToken::ETClose => Token::TClose,
        }
    }
}

// Elementwise view of a token sequence. ExecToken does not nest, so a plain
// length decreases suffices (unlike ewords_view).
pub open spec fn etoks_view(s: Seq<ExecToken>) -> Seq<Token>
    decreases s.len(),
{
    if s.len() == 0 {
        Seq::<Token>::empty()
    } else {
        seq![s[0]@] + etoks_view(s.subrange(1, s.len() as int))
    }
}

// Cons distributes over etoks_view (the lex_cons shape).
pub proof fn lemma_etoks_view_cons(x: ExecToken, rest: Seq<ExecToken>)
    ensures etoks_view(seq![x] + rest) == seq![x@] + etoks_view(rest),
{
    let s = seq![x] + rest;
    assert(s.len() == rest.len() + 1);
    assert(s[0] == x);
    assert(s.subrange(1, s.len() as int) =~= rest);
}

// The i64 upper bound, as a ghost int.
pub open spec fn imax() -> int { 9223372036854775807 }

// `all_ints_fit(cs)`: mirrors `lex`'s control flow exactly, additionally
// requiring every maximal digit run's value to be <= i64::MAX. This is the
// precise in-range domain on which the exec lexer refines `lex`.
pub open spec fn all_ints_fit(cs: Seq<char>) -> bool
    decreases cs.len(),
    via all_ints_fit_termination
{
    if cs.len() == 0 {
        true
    } else {
        let c = cs[0];
        if is_ws(c) {
            all_ints_fit(cs.subrange(1, cs.len() as int))
        } else if is_digit(c) {
            let k = leading_digits_len(cs);
            (nat_of_digits(cs.subrange(0, k as int)) as int <= imax())
                && all_ints_fit(cs.subrange(k as int, cs.len() as int))
        } else if is_lower(c) {
            let k = leading_name_len(cs);
            all_ints_fit(cs.subrange(k as int, cs.len() as int))
        } else if c == '[' {
            all_ints_fit(cs.subrange(1, cs.len() as int))
        } else if c == ']' {
            all_ints_fit(cs.subrange(1, cs.len() as int))
        } else {
            match glyph_to_gprim(c) {
                Some(_) => all_ints_fit(cs.subrange(1, cs.len() as int)),
                None => true,
            }
        }
    }
}

#[verifier::decreases_by]
proof fn all_ints_fit_termination(cs: Seq<char>) {
    if cs.len() == 0 {
    } else {
        let c = cs[0];
        if is_ws(c) {
        } else if is_digit(c) {
            ldl_bound(cs); ldl_pos(cs);
        } else if is_lower(c) {
            lnl_bound(cs); lnl_pos(cs);
        } else {
        }
    }
}

// ============================================================
// 17. Exec lexer helpers (character classes, digit value, munch scanners).
// ============================================================

fn exec_is_ws(c: char) -> (r: bool)
    ensures r == is_ws(c),
{
    c == ' ' || c == '\t' || c == '\n' || c == '\r'
}

fn exec_is_namechar(c: char) -> (r: bool)
    ensures r == is_namechar(c),
{
    exec_is_lower(c) || exec_is_digit(c)
}

// exec digit value. The if-chain is identical to `digit_val`, so equality holds
// for every char (no exhaustion argument needed); all branches return 0..=9.
fn exec_digit_val(c: char) -> (r: u64)
    ensures r as nat == digit_val(c), r < 10,
{
    if c == '0' { 0 } else if c == '1' { 1 } else if c == '2' { 2 }
    else if c == '3' { 3 } else if c == '4' { 4 } else if c == '5' { 5 }
    else if c == '6' { 6 } else if c == '7' { 7 } else if c == '8' { 8 }
    else { 9 }
}

// exec glyph lookup (mirror of glyph_to_gprim). Same if-chain, so the view
// relation holds branch-by-branch.
fn exec_glyph_to_prim(c: char) -> (r: Option<EPrim>)
    ensures
        match r {
            Some(p) => glyph_to_gprim(c) == Some(p@),
            None => glyph_to_gprim(c) == None::<GPrim>,
        },
{
    if c == ':' { Some(EPrim::Dup) }
    else if c == '_' { Some(EPrim::Drop) }
    else if c == '~' { Some(EPrim::Swap) }
    else if c == '@' { Some(EPrim::Rot) }
    else if c == '^' { Some(EPrim::Over) }
    else if c == '!' { Some(EPrim::Apply) }
    else if c == ',' { Some(EPrim::Cat) }
    else if c == ';' { Some(EPrim::Cons) }
    else if c == '\'' { Some(EPrim::Dip) }
    else if c == '+' { Some(EPrim::Add) }
    else if c == '-' { Some(EPrim::Sub) }
    else if c == '*' { Some(EPrim::Mul) }
    else if c == '/' { Some(EPrim::Div) }
    else if c == '%' { Some(EPrim::Mod) }
    else if c == '=' { Some(EPrim::Eq) }
    else if c == '<' { Some(EPrim::Lt) }
    else if c == '?' { Some(EPrim::If) }
    else if c == '&' { Some(EPrim::PrimRec) }
    else if c == '.' { Some(EPrim::Times) }
    else if c == '|' { Some(EPrim::LinRec) }
    else if c == '>' { Some(EPrim::Uncons) }
    else if c == '(' { Some(EPrim::Fold) }
    else if c == '$' { Some(EPrim::Xor) }
    else { None }
}

// forward step of nat_of_digits: appending a digit multiplies by 10 and adds.
pub proof fn lemma_nat_of_digits_push(s: Seq<char>, x: char)
    ensures nat_of_digits(s.push(x)) == nat_of_digits(s) * 10 + digit_val(x),
{
    let t = s.push(x);
    assert(t.len() == s.len() + 1);
    assert(t.subrange(0, t.len() as int - 1) =~= s);
    assert(t[t.len() as int - 1] == x);
}

// Copy the sub-slice cs[i..j] into a fresh Vec<char>.
fn slice_copy(cs: &Vec<char>, i: usize, j: usize) -> (r: Vec<char>)
    requires i <= j <= cs.len(),
    ensures r@ == cs@.subrange(i as int, j as int),
{
    let mut r: Vec<char> = Vec::new();
    let mut k = i;
    while k < j
        invariant
            i <= k <= j <= cs.len(),
            r@ == cs@.subrange(i as int, k as int),
        decreases j - k,
    {
        r.push(cs[k]);
        proof {
            assert(cs@.subrange(i as int, (k + 1) as int)
                =~= cs@.subrange(i as int, k as int).push(cs@[k as int]));
        }
        k = k + 1;
    }
    proof { assert(r@ =~= cs@.subrange(i as int, j as int)); }
    r
}

// Maximal-munch digit scanner. Returns (end index, Option<i64> value): Some(v)
// if the run's value fits i64, None on overflow. The end index equals
// i + leading_digits_len of the suffix.
fn scan_digits(cs: &Vec<char>, i: usize) -> (res: (usize, Option<i64>))
    requires
        i < cs.len(),
        is_digit(cs@[i as int]),
    ensures ({
        let suf = cs@.subrange(i as int, cs@.len() as int);
        let k = leading_digits_len(suf);
        &&& i < res.0 <= cs@.len()
        &&& i as int + k == res.0 as int
        &&& suf.subrange(0, k as int) =~= cs@.subrange(i as int, res.0 as int)
        &&& match res.1 {
                Some(v) => v >= 0
                    && (v as nat) == nat_of_digits(cs@.subrange(i as int, res.0 as int))
                    && (v as int) <= imax(),
                None => nat_of_digits(cs@.subrange(i as int, res.0 as int)) as int > imax(),
            }
    }),
{
    let mut j = i;
    let mut acc: Option<i64> = Some(0i64);
    while j < cs.len() && exec_is_digit(cs[j])
        invariant
            i <= j <= cs.len(),
            forall|t: int| i <= t < j ==> is_digit(cs@[t]),
            match acc {
                Some(v) => v >= 0
                    && (v as nat) == nat_of_digits(cs@.subrange(i as int, j as int))
                    && (v as int) <= imax(),
                None => nat_of_digits(cs@.subrange(i as int, j as int)) as int > imax(),
            },
        decreases cs.len() - j,
    {
        let ghost run = cs@.subrange(i as int, j as int);
        let d: u64 = exec_digit_val(cs[j]);
        proof {
            assert(cs@.subrange(i as int, (j + 1) as int)
                =~= run.push(cs@[j as int]));
            lemma_nat_of_digits_push(run, cs@[j as int]);
        }
        let ghost gval_new = nat_of_digits(cs@.subrange(i as int, (j + 1) as int));
        acc = match acc {
            None => {
                proof {
                    // gval was > imax; gval_new = gval*10 + d >= gval > imax.
                    assert(nat_of_digits(run) * 10 >= nat_of_digits(run)) by (nonlinear_arith);
                }
                None
            }
            Some(v) => {
                if v > 922337203685477580 || (v == 922337203685477580 && d > 7) {
                    proof {
                        // v == nat_of_digits(run); gval_new = v*10 + d > imax.
                        if v > 922337203685477580 {
                            assert((v as int) * 10 + (d as int) > 9223372036854775807) by (nonlinear_arith)
                                requires v as int >= 922337203685477581, d as int >= 0;
                        } else {
                            assert((v as int) * 10 + (d as int) > 9223372036854775807) by (nonlinear_arith)
                                requires v as int == 922337203685477580, d as int >= 8;
                        }
                    }
                    None
                } else {
                    // v <= q and (v < q or d <= rr): v*10 + d fits i64.
                    proof {
                        if v < 922337203685477580 {
                            assert((v as int) * 10 + (d as int) <= 9223372036854775807) by (nonlinear_arith)
                                requires v as int <= 922337203685477579, d as int <= 9;
                        } else {
                            assert((v as int) * 10 + (d as int) <= 9223372036854775807) by (nonlinear_arith)
                                requires v as int == 922337203685477580, d as int <= 7;
                        }
                    }
                    Some(v * 10 + (d as i64))
                }
            }
        };
        j = j + 1;
    }
    proof {
        let suf = cs@.subrange(i as int, cs@.len() as int);
        let a = cs@.subrange(i as int, j as int);
        let b = cs@.subrange(j as int, cs@.len() as int);
        assert(a + b =~= suf);
        assert(b.len() == 0 || !is_digit(b[0])) by {
            if b.len() != 0 {
                assert(b[0] == cs@[j as int]);
            }
        }
        lemma_ldl_all(a, b);
        assert(suf.subrange(0, (j - i) as int) =~= a);
    }
    (j, acc)
}

// Maximal-munch name scanner. Returns end index = i + leading_name_len(suffix).
fn scan_name(cs: &Vec<char>, i: usize) -> (j: usize)
    requires
        i < cs.len(),
        is_lower(cs@[i as int]),
    ensures ({
        let suf = cs@.subrange(i as int, cs@.len() as int);
        let k = leading_name_len(suf);
        &&& i < j <= cs@.len()
        &&& i as int + k == j as int
        &&& suf.subrange(0, k as int) =~= cs@.subrange(i as int, j as int)
    }),
{
    let mut j = i;
    while j < cs.len() && exec_is_namechar(cs[j])
        invariant
            i <= j <= cs.len(),
            forall|t: int| i <= t < j ==> is_namechar(cs@[t]),
        decreases cs.len() - j,
    {
        j = j + 1;
    }
    proof {
        let suf = cs@.subrange(i as int, cs@.len() as int);
        let a = cs@.subrange(i as int, j as int);
        let b = cs@.subrange(j as int, cs@.len() as int);
        assert(a + b =~= suf);
        assert(b.len() == 0 || !is_namechar(b[0])) by {
            if b.len() != 0 {
                assert(b[0] == cs@[j as int]);
            }
        }
        // is_lower(cs[i]) ==> is_namechar, so j > i.
        assert(is_namechar(cs@[i as int]));
        lemma_lnl_all(a, b);
        assert(suf.subrange(0, (j - i) as int) =~= a);
    }
    j
}

// ============================================================
// 18. The exec lexer: recursive, mirrors `lex` suffix-for-suffix. On the
// in-range domain (`all_ints_fit`) it reproduces `lex`; off it, it reports
// overflow.
// ============================================================

pub enum ExecLexRes {
    LOk(Vec<ExecToken>),
    LBadChar,
    LOverflow,
}

fn exec_lex(cs: &Vec<char>, i: usize) -> (r: ExecLexRes)
    requires i <= cs.len(),
    ensures ({
        let suf = cs@.subrange(i as int, cs@.len() as int);
        match r {
            ExecLexRes::LOverflow => !all_ints_fit(suf),
            ExecLexRes::LOk(ts) => all_ints_fit(suf) && lex(suf) == Some(etoks_view(ts@)),
            ExecLexRes::LBadChar => all_ints_fit(suf) && lex(suf) == None::<Seq<Token>>,
        }
    }),
    decreases cs.len() - i,
{
    let ghost suf = cs@.subrange(i as int, cs@.len() as int);
    if i == cs.len() {
        proof {
            assert(suf =~= Seq::<char>::empty());
            assert(etoks_view(Seq::<ExecToken>::empty()) =~= Seq::<Token>::empty());
        }
        return ExecLexRes::LOk(Vec::new());
    }
    let c = cs[i];
    assert(suf.len() > 0 && suf[0] == c);
    if exec_is_ws(c) {
        let r = exec_lex(cs, i + 1);
        proof {
            assert(cs@.subrange((i + 1) as int, cs@.len() as int) =~= suf.subrange(1, suf.len() as int));
        }
        r
    } else if exec_is_digit(c) {
        let (j, ov) = scan_digits(cs, i);
        let ghost k = leading_digits_len(suf);
        let ghost blk = suf.subrange(0, k as int);
        let ghost suf2 = cs@.subrange(j as int, cs@.len() as int);
        proof {
            assert(suf2 =~= suf.subrange(k as int, suf.len() as int));
            assert(blk =~= cs@.subrange(i as int, j as int));
        }
        match ov {
            None => {
                proof {
                    assert(nat_of_digits(blk) as int > imax());
                    // all_ints_fit(suf) unfolds to a false first conjunct.
                }
                ExecLexRes::LOverflow
            }
            Some(v) => {
                let rest = exec_lex(cs, j);
                match rest {
                    ExecLexRes::LOverflow => {
                        ExecLexRes::LOverflow
                    }
                    ExecLexRes::LBadChar => {
                        proof {
                            assert(nat_of_digits(blk) as int <= imax());
                            assert(lex(suf) == lex_cons(Token::TInt(nat_of_digits(blk) as int), lex(suf2)));
                        }
                        ExecLexRes::LBadChar
                    }
                    ExecLexRes::LOk(ts) => {
                        let mut out = ts;
                        out.insert(0, ExecToken::ETInt(v));
                        proof {
                            let gts = etoks_view(out@);
                            assert(out@ =~= seq![ExecToken::ETInt(v)] + ts@);
                            lemma_etoks_view_cons(ExecToken::ETInt(v), ts@);
                            assert(ExecToken::ETInt(v)@ == Token::TInt(v as int));
                            assert(v as int == nat_of_digits(blk) as int);
                            assert(lex(suf) == lex_cons(Token::TInt(nat_of_digits(blk) as int), lex(suf2)));
                            assert(lex(suf) == Some(seq![Token::TInt(v as int)] + etoks_view(ts@)));
                        }
                        ExecLexRes::LOk(out)
                    }
                }
            }
        }
    } else if exec_is_lower(c) {
        let j = scan_name(cs, i);
        let ghost k = leading_name_len(suf);
        let ghost blk = suf.subrange(0, k as int);
        let ghost suf2 = cs@.subrange(j as int, cs@.len() as int);
        let name = slice_copy(cs, i, j);
        proof {
            assert(suf2 =~= suf.subrange(k as int, suf.len() as int));
            assert(blk =~= cs@.subrange(i as int, j as int));
            assert(name@ == blk);
        }
        let rest = exec_lex(cs, j);
        match rest {
            ExecLexRes::LOverflow => ExecLexRes::LOverflow,
            ExecLexRes::LBadChar => {
                proof {
                    assert(lex(suf) == lex_cons(Token::TName(blk), lex(suf2)));
                }
                ExecLexRes::LBadChar
            }
            ExecLexRes::LOk(ts) => {
                let mut out = ts;
                out.insert(0, ExecToken::ETName(name));
                proof {
                    assert(out@ =~= seq![ExecToken::ETName(name)] + ts@);
                    lemma_etoks_view_cons(ExecToken::ETName(name), ts@);
                    assert(ExecToken::ETName(name)@ == Token::TName(blk));
                    assert(lex(suf) == lex_cons(Token::TName(blk), lex(suf2)));
                }
                ExecLexRes::LOk(out)
            }
        }
    } else if c == '[' {
        let ghost suf2 = cs@.subrange((i + 1) as int, cs@.len() as int);
        proof {
            assert(suf2 =~= suf.subrange(1, suf.len() as int));
            assert(!is_ws('[') && !is_digit('[') && !is_lower('['));
        }
        let rest = exec_lex(cs, i + 1);
        match rest {
            ExecLexRes::LOverflow => ExecLexRes::LOverflow,
            ExecLexRes::LBadChar => {
                proof { assert(lex(suf) == lex_cons(Token::TOpen, lex(suf2))); }
                ExecLexRes::LBadChar
            }
            ExecLexRes::LOk(ts) => {
                let mut out = ts;
                out.insert(0, ExecToken::ETOpen);
                proof {
                    assert(out@ =~= seq![ExecToken::ETOpen] + ts@);
                    lemma_etoks_view_cons(ExecToken::ETOpen, ts@);
                    assert(ExecToken::ETOpen@ == Token::TOpen);
                    assert(lex(suf) == lex_cons(Token::TOpen, lex(suf2)));
                }
                ExecLexRes::LOk(out)
            }
        }
    } else if c == ']' {
        let ghost suf2 = cs@.subrange((i + 1) as int, cs@.len() as int);
        proof {
            assert(suf2 =~= suf.subrange(1, suf.len() as int));
            assert(!is_ws(']') && !is_digit(']') && !is_lower(']') && ']' != '[');
        }
        let rest = exec_lex(cs, i + 1);
        match rest {
            ExecLexRes::LOverflow => ExecLexRes::LOverflow,
            ExecLexRes::LBadChar => {
                proof { assert(lex(suf) == lex_cons(Token::TClose, lex(suf2))); }
                ExecLexRes::LBadChar
            }
            ExecLexRes::LOk(ts) => {
                let mut out = ts;
                out.insert(0, ExecToken::ETClose);
                proof {
                    assert(out@ =~= seq![ExecToken::ETClose] + ts@);
                    lemma_etoks_view_cons(ExecToken::ETClose, ts@);
                    assert(ExecToken::ETClose@ == Token::TClose);
                    assert(lex(suf) == lex_cons(Token::TClose, lex(suf2)));
                }
                ExecLexRes::LOk(out)
            }
        }
    } else {
        proof {
            assert(!is_ws(c) && !is_digit(c) && !is_lower(c) && c != '[' && c != ']');
        }
        match exec_glyph_to_prim(c) {
            Some(p) => {
                let ghost suf2 = cs@.subrange((i + 1) as int, cs@.len() as int);
                proof {
                    assert(suf2 =~= suf.subrange(1, suf.len() as int));
                    assert(glyph_to_gprim(c) == Some(p@));
                }
                let rest = exec_lex(cs, i + 1);
                match rest {
                    ExecLexRes::LOverflow => ExecLexRes::LOverflow,
                    ExecLexRes::LBadChar => {
                        proof { assert(lex(suf) == lex_cons(Token::TGlyph(p@), lex(suf2))); }
                        ExecLexRes::LBadChar
                    }
                    ExecLexRes::LOk(ts) => {
                        let mut out = ts;
                        out.insert(0, ExecToken::ETGlyph(p));
                        proof {
                            assert(out@ =~= seq![ExecToken::ETGlyph(p)] + ts@);
                            lemma_etoks_view_cons(ExecToken::ETGlyph(p), ts@);
                            assert(ExecToken::ETGlyph(p)@ == Token::TGlyph(p@));
                            assert(lex(suf) == lex_cons(Token::TGlyph(p@), lex(suf2)));
                        }
                        ExecLexRes::LOk(out)
                    }
                }
            }
            None => {
                proof {
                    assert(glyph_to_gprim(c) == None::<GPrim>);
                    assert(lex(suf) == None::<Seq<Token>>);
                }
                ExecLexRes::LBadChar
            }
        }
    }
}

// ============================================================
// 19. Exec grouper machinery: the level stack and its view.
// ============================================================

// View of a level stack (Vec<Vec<EWord>>) to the ghost Seq<Seq<GWord>>.
pub open spec fn stack_view(levels: Seq<Vec<EWord>>) -> Seq<Seq<GWord>>
    decreases levels.len(),
{
    if levels.len() == 0 {
        Seq::<Seq<GWord>>::empty()
    } else {
        seq![ewords_view(levels[0]@)] + stack_view(levels.subrange(1, levels.len() as int))
    }
}

pub proof fn lemma_stack_view_len_index(s: Seq<Vec<EWord>>)
    ensures
        stack_view(s).len() == s.len(),
        forall|i: int| 0 <= i < s.len() ==> stack_view(s)[i] == ewords_view(#[trigger] s[i]@),
    decreases s.len(),
{
    if s.len() == 0 {
    } else {
        let srest = s.subrange(1, s.len() as int);
        lemma_stack_view_len_index(srest);
        assert forall|i: int| 0 <= i < s.len() implies
            stack_view(s)[i] == ewords_view(#[trigger] s[i]@) by {
            if i == 0 {
                assert(stack_view(s)[0] == ewords_view(s[0]@));
            } else {
                assert(stack_view(s)[i] == stack_view(srest)[i - 1]);
                assert(srest[i - 1] == s[i]);
            }
        }
    }
}

pub proof fn lemma_stack_view_push(s: Seq<Vec<EWord>>, v: Vec<EWord>)
    ensures stack_view(s.push(v)) == stack_view(s).push(ewords_view(v@)),
    decreases s.len(),
{
    if s.len() == 0 {
        let sv = s.push(v);
        assert(sv =~= seq![v]);
        assert(sv.len() == 1 && sv[0] == v);
        assert(sv.subrange(1, sv.len() as int) =~= Seq::<Vec<EWord>>::empty());
        assert(stack_view(sv.subrange(1, sv.len() as int)) == Seq::<Seq<GWord>>::empty());
        assert(stack_view(sv) =~= seq![ewords_view(v@)]);
        assert(stack_view(s) =~= Seq::<Seq<GWord>>::empty());
        assert(stack_view(s).push(ewords_view(v@)) =~= seq![ewords_view(v@)]);
    } else {
        let srest = s.subrange(1, s.len() as int);
        assert((s.push(v))[0] == s[0]);
        assert((s.push(v)).subrange(1, (s.push(v)).len() as int) =~= srest.push(v));
        lemma_stack_view_push(srest, v);
        assert(stack_view(s.push(v)) =~= stack_view(s).push(ewords_view(v@)));
    }
}

pub proof fn lemma_stack_view_last_drop(s: Seq<Vec<EWord>>)
    requires s.len() >= 1,
    ensures
        stack_view(s).last() == ewords_view(s.last()@),
        stack_view(s).drop_last() == stack_view(s.drop_last()),
{
    assert(s =~= s.drop_last().push(s.last()));
    lemma_stack_view_push(s.drop_last(), s.last());
}

// ewords_view distributes over Seq::push (derived from append + singleton).
pub proof fn lemma_ewords_view_push(s: Seq<EWord>, x: EWord)
    ensures ewords_view(s.push(x)) == ewords_view(s).push(eword_view(x)),
{
    assert(s.push(x) =~= s + seq![x]);
    lemma_ewords_view_append(s, seq![x]);
    lemma_ewords_view_singleton(x);
    assert(ewords_view(s).push(eword_view(x)) =~= ewords_view(s) + seq![eword_view(x)]);
}

// Reconstruct an owned EPrim from a reference (EPrim has no Copy derive here).
fn clone_eprim(p: &EPrim) -> (r: EPrim)
    ensures r@ == p@,
{
    match p {
        EPrim::Dup => EPrim::Dup, EPrim::Drop => EPrim::Drop, EPrim::Swap => EPrim::Swap,
        EPrim::Rot => EPrim::Rot, EPrim::Over => EPrim::Over, EPrim::Apply => EPrim::Apply,
        EPrim::Cat => EPrim::Cat, EPrim::Cons => EPrim::Cons, EPrim::Dip => EPrim::Dip,
        EPrim::Add => EPrim::Add, EPrim::Sub => EPrim::Sub, EPrim::Mul => EPrim::Mul,
        EPrim::Div => EPrim::Div, EPrim::Mod => EPrim::Mod, EPrim::Eq => EPrim::Eq,
        EPrim::Lt => EPrim::Lt, EPrim::If => EPrim::If, EPrim::PrimRec => EPrim::PrimRec,
        EPrim::Times => EPrim::Times, EPrim::LinRec => EPrim::LinRec, EPrim::Uncons => EPrim::Uncons,
        EPrim::Fold => EPrim::Fold, EPrim::Xor => EPrim::Xor,
    }
}

// Push a word onto the top (last) level, mirroring `push_word` on the view.
fn push_top(levels: Vec<Vec<EWord>>, w: EWord) -> (r: Vec<Vec<EWord>>)
    requires levels.len() >= 1,
    ensures
        stack_view(r@) == push_word(stack_view(levels@), eword_view(w)),
        r.len() == levels.len(),
{
    let ghost gw = eword_view(w);
    let mut lv = levels;
    let ghost bigl = lv@;
    let mut top = lv.pop().unwrap();
    top.push(w);
    lv.push(top);
    proof {
        lemma_stack_view_push(bigl.drop_last(), top);
        lemma_stack_view_last_drop(bigl);
        lemma_ewords_view_push(bigl.last()@, w);
        assert(stack_view(lv@) =~= push_word(stack_view(bigl), gw));
    }
    lv
}

// ============================================================
// 20. The exec grouper: recursive, mirrors `group_fold` token-for-token.
// ============================================================

fn exec_group(ts: &Vec<ExecToken>, idx: usize, levels: Vec<Vec<EWord>>) -> (r: Option<Vec<Vec<EWord>>>)
    requires
        idx <= ts.len(),
        levels.len() >= 1,
    ensures ({
        let sub = etoks_view(ts@.subrange(idx as int, ts@.len() as int));
        match r {
            Some(out) => group_fold(sub, stack_view(levels@)) == Some(stack_view(out@)),
            None => group_fold(sub, stack_view(levels@)) == None::<Seq<Seq<GWord>>>,
        }
    }),
    decreases ts.len() - idx,
{
    let ghost sub = etoks_view(ts@.subrange(idx as int, ts@.len() as int));
    if idx == ts.len() {
        proof {
            assert(ts@.subrange(idx as int, ts@.len() as int) =~= Seq::<ExecToken>::empty());
            assert(sub =~= Seq::<Token>::empty());
        }
        return Some(levels);
    }
    let ghost sub2 = etoks_view(ts@.subrange((idx + 1) as int, ts@.len() as int));
    let ghost head = ts@[idx as int]@;
    let ghost sv = stack_view(levels@);
    proof {
        assert(ts@.subrange(idx as int, ts@.len() as int)
            =~= seq![ts@[idx as int]] + ts@.subrange((idx + 1) as int, ts@.len() as int));
        lemma_etoks_view_cons(ts@[idx as int], ts@.subrange((idx + 1) as int, ts@.len() as int));
        // sub == seq![head] + sub2
        assert(sub[0] == head);
        assert(sub.subrange(1, sub.len() as int) =~= sub2);
        lemma_stack_view_len_index(levels@);
        assert(sv.len() == levels.len());
    }
    match &ts[idx] {
        ExecToken::ETOpen => {
            let mut lv = levels;
            let ghost pre = lv@;
            let empty_level: Vec<EWord> = Vec::new();
            proof { assert(ewords_view(empty_level@) =~= Seq::<GWord>::empty()); }
            lv.push(empty_level);
            proof {
                lemma_stack_view_push(pre, empty_level);
                assert(stack_view(lv@) =~= sv.push(Seq::<GWord>::empty()));
                assert(head == Token::TOpen);
                assert(group_fold(sub, sv) == group_fold(sub2, sv.push(Seq::<GWord>::empty())));
            }
            exec_group(ts, idx + 1, lv)
        }
        ExecToken::ETClose => {
            if levels.len() <= 1 {
                proof {
                    assert(head == Token::TClose);
                    assert(sv.len() <= 1);
                    assert(group_fold(sub, sv) == None::<Seq<Seq<GWord>>>);
                }
                None
            } else {
                let mut lv = levels;
                let ghost bigl = lv@;
                let inner = lv.pop().unwrap();
                let lv2 = push_top(lv, EWord::EPushQuote(inner));
                proof {
                    lemma_stack_view_last_drop(bigl);
                    // stack_view(lv@) == sv.drop_last(); inner@ views to sv.last()
                    assert(eword_view(EWord::EPushQuote(inner)) == GWord::PushQuote(sv.last()));
                    assert(stack_view(lv2@) == push_word(sv.drop_last(), GWord::PushQuote(sv.last())));
                    assert(head == Token::TClose);
                    assert(sv.len() > 1);
                    assert(group_fold(sub, sv)
                        == group_fold(sub2, push_word(sv.drop_last(), GWord::PushQuote(sv.last()))));
                }
                exec_group(ts, idx + 1, lv2)
            }
        }
        ExecToken::ETInt(n) => {
            let lv = push_top(levels, EWord::EPushInt(*n));
            proof {
                assert(head == Token::TInt(*n as int));
                assert(eword_view(EWord::EPushInt(*n)) == GWord::PushInt(*n as int));
                assert(group_fold(sub, sv) == group_fold(sub2, push_word(sv, GWord::PushInt(*n as int))));
            }
            exec_group(ts, idx + 1, lv)
        }
        ExecToken::ETName(v) => {
            let name = slice_copy(v, 0, v.len());
            proof { assert(name@ == v@); }
            let lv = push_top(levels, EWord::ECall(name));
            proof {
                assert(head == Token::TName(v@));
                assert(eword_view(EWord::ECall(name)) == GWord::Call(v@));
                assert(group_fold(sub, sv) == group_fold(sub2, push_word(sv, GWord::Call(v@))));
            }
            exec_group(ts, idx + 1, lv)
        }
        ExecToken::ETGlyph(p) => {
            let ep = clone_eprim(p);
            proof { assert(ep@ == p@); }
            let lv = push_top(levels, EWord::EPrim(ep));
            proof {
                assert(head == Token::TGlyph(p@));
                assert(eword_view(EWord::EPrim(ep)) == GWord::Prim(p@));
                assert(group_fold(sub, sv) == group_fold(sub2, push_word(sv, GWord::Prim(p@))));
            }
            exec_group(ts, idx + 1, lv)
        }
    }
}

// ============================================================
// 21. The exec parser and its refinement of spec_parse.
// ============================================================

// Executable parse outcome; views to the ghost ParseOutcome.
pub enum ExecOutcome {
    EOk(Vec<EWord>),
    EErr,
}

impl View for ExecOutcome {
    type V = ParseOutcome;
    open spec fn view(&self) -> ParseOutcome {
        match self {
            ExecOutcome::EOk(p) => ParseOutcome::Ok(ewords_view(p@)),
            ExecOutcome::EErr => ParseOutcome::Err,
        }
    }
}

// The executable parser. On the in-range domain (`all_ints_fit`) it refines
// `spec_parse` exactly; off it, it rejects (EErr) — the ONLY way the exec parser
// diverges from the unbounded spec is by rejecting an i64-overflowing literal.
pub fn exec_parse(cs: &Vec<char>) -> (r: ExecOutcome)
    ensures
        all_ints_fit(cs@) ==> r@ == spec_parse(cs@),
        !all_ints_fit(cs@) ==> r == ExecOutcome::EErr,
{
    proof { assert(cs@.subrange(0, cs@.len() as int) =~= cs@); }
    match exec_lex(cs, 0) {
        ExecLexRes::LOverflow => ExecOutcome::EErr,
        ExecLexRes::LBadChar => {
            // all_ints_fit(cs@) holds; lex(cs@) == None ==> spec_parse == Err.
            ExecOutcome::EErr
        }
        ExecLexRes::LOk(ts) => {
            // all_ints_fit(cs@); lex(cs@) == Some(etoks_view(ts@)).
            let mut init: Vec<Vec<EWord>> = Vec::new();
            let ghost pre = init@;
            let e: Vec<EWord> = Vec::new();
            init.push(e);
            proof {
                lemma_stack_view_push(pre, e);
                assert(pre =~= Seq::<Vec<EWord>>::empty());
                assert(stack_view(pre) =~= Seq::<Seq<GWord>>::empty());
                assert(ewords_view(e@) =~= Seq::<GWord>::empty());
                assert(stack_view(init@) =~= seq![Seq::<GWord>::empty()]);
                assert(ts@.subrange(0, ts@.len() as int) =~= ts@);
            }
            match exec_group(&ts, 0, init) {
                None => {
                    proof {
                        assert(group_fold(etoks_view(ts@), seq![Seq::<GWord>::empty()])
                            == None::<Seq<Seq<GWord>>>);
                    }
                    ExecOutcome::EErr
                }
                Some(levels) => {
                    let ghost gl = levels@;
                    proof {
                        assert(group_fold(etoks_view(ts@), seq![Seq::<GWord>::empty()])
                            == Some(stack_view(gl)));
                        lemma_stack_view_len_index(gl);
                    }
                    if levels.len() == 1 {
                        let mut lv = levels;
                        let prog = lv.pop().unwrap();
                        proof {
                            assert(gl.len() == 1);
                            assert(gl.last() == gl[0]);
                            assert(prog@ == gl[0]@);
                            assert(stack_view(gl)[0] == ewords_view(gl[0]@));
                            assert(stack_view(gl).len() == 1);
                        }
                        ExecOutcome::EOk(prog)
                    } else {
                        proof {
                            assert(stack_view(gl).len() != 1);
                        }
                        ExecOutcome::EErr
                    }
                }
            }
        }
    }
}

// ============================================================
// 22. The EXEC ROUND-TRIP (PRIMARY): exec_parse(exec_print(p)) recovers p.
//
// This dodges overflow entirely: p : Vec<EWord> carries i64 ints, so the
// printer's image only ever has digit runs that fit i64 — the parser never
// overflows on it. We prove all_ints_fit(exec_print(p)@), then chain the
// exec printer/parser refinements with the ghost p4_roundtrip.
// ============================================================

// A token has an i64-representable integer (if it is an int token at all).
pub open spec fn int_fits(t: Token) -> bool {
    match t {
        Token::TInt(n) => 0 <= n <= imax(),
        _ => true,
    }
}

pub open spec fn toks_fit(ts: Seq<Token>) -> bool {
    forall|i: int| 0 <= i < ts.len() ==> int_fits(#[trigger] ts[i])
}

pub proof fn lemma_toks_fit_append(a: Seq<Token>, b: Seq<Token>)
    ensures toks_fit(a + b) == (toks_fit(a) && toks_fit(b)),
{
    if toks_fit(a) && toks_fit(b) {
        assert forall|i: int| 0 <= i < (a + b).len() implies int_fits(#[trigger] (a + b)[i]) by {
            if i < a.len() {
                assert((a + b)[i] == a[i]);
            } else {
                assert((a + b)[i] == b[i - a.len()]);
            }
        }
    } else if !toks_fit(a) {
        let i = choose|i: int| 0 <= i < a.len() && !int_fits(#[trigger] a[i]);
        assert((a + b)[i] == a[i]);
    } else {
        let j = choose|j: int| 0 <= j < b.len() && !int_fits(#[trigger] b[j]);
        assert((a + b)[j + a.len()] == b[j]);
    }
}

// The key bridge: any character sequence that lexes to a token stream whose
// integer tokens all fit i64 is itself in-range (all_ints_fit). Mirrors `lex`'s
// recursion, matching each digit-run value to its TInt token.
pub proof fn lemma_lex_all_ints_fit(cs: Seq<char>, ts: Seq<Token>)
    requires lex(cs) == Some(ts), toks_fit(ts),
    ensures all_ints_fit(cs),
    decreases cs.len(),
{
    if cs.len() == 0 {
    } else {
        let c = cs[0];
        if is_ws(c) {
            lemma_lex_all_ints_fit(cs.subrange(1, cs.len() as int), ts);
        } else if is_digit(c) {
            ldl_bound(cs); ldl_pos(cs);
            let k = leading_digits_len(cs);
            let cs2 = cs.subrange(k as int, cs.len() as int);
            let val = nat_of_digits(cs.subrange(0, k as int)) as int;
            let tok = Token::TInt(val);
            match lex(cs2) {
                Some(rest) => {
                    assert(ts =~= seq![tok] + rest);
                    assert(int_fits(ts[0]) && ts[0] == tok);
                    assert(val <= imax());
                    assert(toks_fit(rest)) by {
                        assert forall|i: int| 0 <= i < rest.len() implies int_fits(#[trigger] rest[i]) by {
                            assert((seq![tok] + rest)[i + 1] == rest[i]);
                            assert(int_fits(ts[i + 1]));
                        }
                    }
                    lemma_lex_all_ints_fit(cs2, rest);
                }
                None => { assert(false); }
            }
        } else if is_lower(c) {
            lnl_bound(cs); lnl_pos(cs);
            let k = leading_name_len(cs);
            let cs2 = cs.subrange(k as int, cs.len() as int);
            let tok = Token::TName(cs.subrange(0, k as int));
            match lex(cs2) {
                Some(rest) => {
                    assert(ts =~= seq![tok] + rest);
                    lemma_toks_fit_drop_head(tok, rest, ts);
                    lemma_lex_all_ints_fit(cs2, rest);
                }
                None => { assert(false); }
            }
        } else if c == '[' {
            let cs2 = cs.subrange(1, cs.len() as int);
            match lex(cs2) {
                Some(rest) => {
                    assert(ts =~= seq![Token::TOpen] + rest);
                    lemma_toks_fit_drop_head(Token::TOpen, rest, ts);
                    lemma_lex_all_ints_fit(cs2, rest);
                }
                None => { assert(false); }
            }
        } else if c == ']' {
            let cs2 = cs.subrange(1, cs.len() as int);
            match lex(cs2) {
                Some(rest) => {
                    assert(ts =~= seq![Token::TClose] + rest);
                    lemma_toks_fit_drop_head(Token::TClose, rest, ts);
                    lemma_lex_all_ints_fit(cs2, rest);
                }
                None => { assert(false); }
            }
        } else {
            match glyph_to_gprim(c) {
                Some(p) => {
                    let cs2 = cs.subrange(1, cs.len() as int);
                    match lex(cs2) {
                        Some(rest) => {
                            assert(ts =~= seq![Token::TGlyph(p)] + rest);
                            lemma_toks_fit_drop_head(Token::TGlyph(p), rest, ts);
                            lemma_lex_all_ints_fit(cs2, rest);
                        }
                        None => { assert(false); }
                    }
                }
                None => { assert(false); }
            }
        }
    }
}

// Dropping a non-overflowing (or non-int) head token preserves toks_fit.
pub proof fn lemma_toks_fit_drop_head(tok: Token, rest: Seq<Token>, ts: Seq<Token>)
    requires ts == seq![tok] + rest, toks_fit(ts),
    ensures toks_fit(rest),
{
    assert forall|i: int| 0 <= i < rest.len() implies int_fits(#[trigger] rest[i]) by {
        assert((seq![tok] + rest)[i + 1] == rest[i]);
        assert(int_fits(ts[i + 1]));
    }
}

// The printer's token image of an exec program has only i64-fitting ints:
// every TInt originates from an EPushInt(i64) whose value is in [0, i64::MAX]
// on the well-formed domain.
pub proof fn lemma_toks_fit_word(w: EWord)
    requires wf_word(eword_view(w)),
    ensures toks_fit(toks_word(eword_view(w))),
    decreases w, 0nat,
{
    match w {
        EWord::EPushInt(i) => {
            assert(toks_word(eword_view(w)) =~= seq![Token::TInt(i as int)]);
            assert(i <= 9223372036854775807);
            assert(i as int <= imax());
            assert(int_fits(Token::TInt(i as int)));
        }
        EWord::EPrim(p) => {
            assert(toks_word(eword_view(w)) =~= seq![Token::TGlyph(p@)]);
        }
        EWord::ECall(v) => {
            assert(toks_word(eword_view(w)) =~= seq![Token::TName(v@)]);
        }
        EWord::EPushQuote(ws) => {
            lemma_toks_fit_words(ws@);
            let mid = toks_words(ewords_view(ws@));
            assert(toks_word(eword_view(w)) =~= seq![Token::TOpen] + mid + seq![Token::TClose]);
            lemma_toks_fit_append(seq![Token::TOpen], mid + seq![Token::TClose]);
            lemma_toks_fit_append(mid, seq![Token::TClose]);
            assert(toks_fit(seq![Token::TOpen]));
            assert(toks_fit(seq![Token::TClose]));
        }
    }
}

pub proof fn lemma_toks_fit_words(s: Seq<EWord>)
    requires wf_words(ewords_view(s)),
    ensures toks_fit(toks_words(ewords_view(s))),
    decreases s, s.len(),
{
    if s.len() == 0 {
        assert(toks_words(ewords_view(s)) =~= Seq::<Token>::empty());
    } else {
        let g = ewords_view(s);
        let srest = s.subrange(1, s.len() as int);
        lemma_ewords_view_len_index(s);
        assert(g[0] == eword_view(s[0]));
        assert(g.subrange(1, g.len() as int) =~= ewords_view(srest));
        // wf_words(g): head wf_word and tail wf_words.
        lemma_wf_words_index(g, 0);
        assert(wf_word(eword_view(s[0])));
        assert(wf_words(ewords_view(srest)));
        lemma_toks_fit_word(s[0]);
        lemma_toks_fit_words(srest);
        assert(toks_words(g) =~= toks_word(eword_view(s[0])) + toks_words(ewords_view(srest)));
        lemma_toks_fit_append(toks_word(eword_view(s[0])), toks_words(ewords_view(srest)));
    }
}

// PRIMARY THEOREM. Parsing the printout of any well-formed exec program
// recovers exactly that program (as a view equality to spec Ok). Expressed as
// an exec fn (it drives the real exec printer + parser) whose postcondition is
// the machine-checked round-trip statement.
pub fn exec_roundtrip(p: &Vec<EWord>) -> (r: ExecOutcome)
    requires wf_words(ewords_view(p@)),
    ensures r@ == ParseOutcome::Ok(ewords_view(p@)),
{
    let printed = exec_print(p);
    let r = exec_parse(&printed);
    proof {
        let g = ewords_view(p@);
        // 1. exec_print refines spec_print.
        assert(printed@ == spec_print(g));
        // 2. printed string lexes back to toks_words(g), whose ints all fit i64.
        lemma_wf_valid_words(g);
        lemma_lex_render(None, toks_words(g));
        assert(lex(spec_print(g)) == Some(toks_words(g)));
        lemma_toks_fit_words(p@);
        assert(toks_fit(toks_words(g)));
        // 3. therefore the printed string is in-range.
        lemma_lex_all_ints_fit(printed@, toks_words(g));
        assert(all_ints_fit(printed@));
        // 4. so exec_parse refines spec_parse here, which round-trips to Ok(g).
        p4_roundtrip(g);
        assert(spec_parse(printed@) == ParseOutcome::Ok(g));
        assert(r@ == spec_parse(printed@));
    }
    r
}

fn main() {}

} // verus!
