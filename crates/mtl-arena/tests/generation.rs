//! Generational memory: a high-water [`Mark`] + truncate-reset reclaims a whole
//! generation, static code below the mark survives, and a winner reified out
//! before the reset is unaffected (design §3.2).

use mtl_arena::{arena_step, ProgWord, Prim, Step, Vm, VmState};
use mtl_core::interp as itp;

#[test]
fn generation_reset_reclaims_and_preserves_static_code() {
    let mut vm = Vm::new();

    // Static code allocated BELOW the mark — must survive a reset.
    let static_q = vm.compile(&[ProgWord::PushInt(42)]).expect("static compile");
    let mark = vm.mark();

    // ---- one generation: allocate + run `1 2 +` to Halt ----
    let q = vm
        .compile(&[ProgWord::PushInt(1), ProgWord::PushInt(2), ProgWord::Prim(Prim::Add)])
        .expect("gen compile");
    let mut st = VmState::initial();
    vm.prepend(&mut st, q);
    loop {
        match arena_step(&mut vm, &mut st) {
            Step::Next => {}
            Step::Halt => break,
            other => panic!("unexpected step: {:?}", other),
        }
    }

    // Reify the winner out BEFORE reset (the escape discipline).
    let winner = vm.reify_stack(st.stack);
    assert_eq!(winner, vec![itp::Value::Int(3)]);

    let grown = vm.mark();
    assert!(grown.tape > mark.tape, "generation should have grown the tape");
    assert!(grown.stack_nodes > mark.stack_nodes, "generation should have grown the stack");

    // ---- reclaim the generation ----
    vm.reset_to(mark);
    assert_eq!(vm.mark(), mark, "reset should truncate every arena back to the mark");

    // Static code below the mark is intact and still readable.
    assert_eq!(vm.reify_quote(static_q), vec![ProgWord::PushInt(42)]);

    // The reified winner is owned and unaffected by the reset.
    assert_eq!(winner, vec![itp::Value::Int(3)]);
}
