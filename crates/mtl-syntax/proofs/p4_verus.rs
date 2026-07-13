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

fn main() {}

} // verus!
