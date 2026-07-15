//! The conforming reference host passes every enumerated obligation; each
//! single-violation adversarial host is rejected at exactly its clause.

use mtl_core::host::{CapabilitySig, HostCode};
use mtl_host_conformance::{check_conformance, ConformingHost, DivergeHost, LeakHost, MisdeclareFaultHost, MutatePrefixHost, Obligation, StringsLeakHost, WrongArityHost};

fn sigs() -> Vec<CapabilitySig> {
    vec![
        CapabilitySig { name: "produce".into(), consumes: 0, produces: 1, faults: vec![] },
        CapabilitySig { name: "consume".into(), consumes: 1, produces: 0, faults: vec![HostCode::OutputCapExceeded] },
        CapabilitySig { name: "map".into(), consumes: 1, produces: 1, faults: vec![HostCode::ToolError] },
        CapabilitySig { name: "combine".into(), consumes: 2, produces: 1, faults: vec![] },
    ]
}

#[test]
fn the_conforming_reference_host_passes_every_obligation() {
    let s = sigs();
    let report = check_conformance(move || ConformingHost::from_sigs(&s), &sigs());
    assert!(report.passed(), "conforming host must pass all obligations:\n{}", report.render());
    for o in Obligation::ALL { assert!(report.for_obligation(o).is_some(), "missing obligation {}", o.id()); }
}

#[test]
fn a_wrong_arity_host_is_rejected_at_o1() {
    let s = sigs();
    let report = check_conformance(move || WrongArityHost::from_sigs(&s), &sigs());
    assert!(!report.for_obligation(Obligation::O1Arity).unwrap().passed, "{}", report.render());
}

#[test]
fn a_leak_host_is_rejected_at_o1() {
    let s = sigs();
    let report = check_conformance(move || LeakHost::from_sigs(&s), &sigs());
    assert!(!report.for_obligation(Obligation::O1Arity).unwrap().passed, "{}", report.render());
}

#[test]
fn a_mutate_prefix_host_is_rejected_at_o2() {
    let s = sigs();
    let report = check_conformance(move || MutatePrefixHost::from_sigs(&s), &sigs());
    assert!(!report.for_obligation(Obligation::O2Prefix).unwrap().passed, "{}", report.render());
}

#[test]
fn a_misdeclared_fault_host_is_rejected_at_o4() {
    let s = sigs();
    let report = check_conformance(move || MisdeclareFaultHost::from_sigs(&s), &sigs());
    assert!(!report.for_obligation(Obligation::O4Faults).unwrap().passed, "{}", report.render());
}

#[test]
fn a_diverging_host_is_rejected_at_o5() {
    let report = check_conformance(|| DivergeHost, &sigs());
    assert!(!report.for_obligation(Obligation::O5Termination).unwrap().passed, "{}", report.render());
}

#[test]
fn a_strings_leak_host_degrades_to_an_o1_out_of_shape_rejection() {
    let s = sigs();
    let report = check_conformance(move || StringsLeakHost::from_sigs(&s), &sigs());
    assert!(!report.for_obligation(Obligation::O1Arity).unwrap().passed, "{}", report.render());
}
