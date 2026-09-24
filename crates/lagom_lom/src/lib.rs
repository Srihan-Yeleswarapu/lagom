// Copyright (c) 2026 Srihan Yeleswarapu.
// Source available under the Lagom License (LICENSE.md).

//! The Lagom Observability Model (§26.5) — the M0 public surface.
//!
//! Spec anchor: 00 §26.5 (the event ring, provenance records, and the
//! failure report), D-32 (LOM failure report v1 at M0, on the interpreter
//! backend), doc 13 §5 (`lagom_lom`: "event ring, provenance records,
//! failure report — the interpreter hook").
//!
//! The M0 implementation consolidates the ring's storage into `lagom_mir`
//! (the IR layer owns the event vocabulary its probes emit); this crate is
//! the observability *facade*: one stable import surface for the driver,
//! the runtime shim, and the tests, so the §26.5 contract (bounded ring,
//! provenance records, the same failure-report renderer on the interpreter
//! and the native runtime) is named in one place.

pub use lagom_mir::{
    failure_report, instrument, line_of, EventRing, LomConfig, LomEvent,
};

/// The lesson write-up linked from the failure report (26.5: answer 8 —
/// "the concept" — comes from the `lagom explain` pipeline; the runtime
/// links the *lesson* whose content the failure names).
pub fn concept_link(code: &str) -> Option<&'static str> {
    match code {
        "index" | "bounds" => Some(
            "a list index counts from 0, and stops before the list's size — `at` reads one item that must exist",
        ),
        "convert" | "number" => Some(
            "text and numbers are different kinds of values; `number from` parses text that must look like a number",
        ),
        "divide" | "zero" => Some(
            "dividing by zero has no answer — check the bottom value before dividing",
        ),
        "ask" | "input" => Some(
            "`ask` reads one line; a program that asks more times than there are answers runs out of input",
        ),
        "overflow" => Some(
            "`number` is a 64-bit integer (D-10); values past its largest size cannot be stored honestly",
        ),
        _ => None,
    }
}

/// Add the concept footer to a rendered report when the failure message
/// matches a known lesson (26.5's answer 8). Appends nothing when no lesson
/// matches — an honest report teaches only what it knows.
pub fn with_concept_link(report: &str, message: &str) -> String {
    let m = message.to_ascii_lowercase();
    let code = if m.contains("past the end") || m.contains("negative") {
        "index"
    } else if m.contains("not a number") || m.contains("is not a decimal") {
        "convert"
    } else if m.contains("divide by zero") || m.contains("remainder of zero") {
        "divide"
    } else if m.contains("end of input") {
        "ask"
    } else if m.contains("overflow") {
        "overflow"
    } else {
        return report.to_string();
    };
    match concept_link(code) {
        Some(lesson) => format!("{report}\n— the concept —\n{lesson}\n"),
        None => report.to_string(),
    }
}

/// The ring capacity M0 ships with (26.5's bounded ring — the report shows
/// the *last* `capacity` events, never unbounded memory).
pub const RING_CAPACITY: usize = 256;

/// A fresh dev-mode ring with the M0 capacity.
pub fn dev_ring() -> EventRing {
    EventRing::new(RING_CAPACITY)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lagom_mir::{instrument, LomConfig, MirProgram};

    fn frontend(src: &str) -> MirProgram {
        let (program, diags) = lagom_parser::parse(src);
        assert!(diags.is_empty(), "parse errors: {diags:?}");
        let checked = lagom_sema::check(&program, src);
        assert!(
            !checked.diags.has_errors(),
            "sema errors: {:?}",
            checked
                .diags
                .items
                .iter()
                .map(|d| d.message.clone())
                .collect::<Vec<_>>()
        );
        lagom_mir::lower(lagom_hir::lower(checked))
    }

    const SRC: &str = r#"
function double
    takes number called x
    returns a number
    gives back x plus x

make start equal to 3
make doubled equal to double start
say doubled
"#;

    #[test]
    fn dev_build_records_provenance_events() {
        let mut prog = frontend(SRC);
        instrument(&mut prog, LomConfig::DEV);
        assert!(
            prog.items.iter().any(|i| match i {
                lagom_mir::MirItem::Function(f) => f
                    .blocks
                    .iter()
                    .any(|b| b
                        .instrs
                        .iter()
                        .any(|ins| matches!(ins, lagom_mir::Instr::EventFunctionEntry { .. }))),
                _ => false,
            }),
            "dev instrumentation must add entry events"
        );
    }

    #[test]
    fn release_build_records_nothing() {
        let mut prog = frontend(SRC);
        instrument(&mut prog, LomConfig::RELEASE);
        let probe_free = prog.items.iter().all(|i| match i {
            lagom_mir::MirItem::Function(f)
            | lagom_mir::MirItem::Main(f)
            | lagom_mir::MirItem::Test(f) => f
                .blocks
                .iter()
                .all(|b| b.instrs.iter().all(|ins| !lagom_mir::is_event(ins))),
        });
        assert!(probe_free, "release builds must carry no probe instructions");
    }

    #[test]
    fn ring_is_bounded() {
        let mut ring = dev_ring();
        for k in 0..(RING_CAPACITY * 3) {
            ring.push(LomEvent::Bind {
                name: format!("x{k}"),
                value: k.to_string(),
                site: 0,
            });
        }
        assert_eq!(ring.len(), RING_CAPACITY);
    }

    #[test]
    fn failure_report_names_the_failing_values_origin() {
        let mut ring = dev_ring();
        ring.push(LomEvent::FunctionEntry {
            function: "double".to_string(),
            args: vec![("x".to_string(), "0".to_string())],
        });
        ring.push(LomEvent::Bind {
            name: "doubled".to_string(),
            value: "0".to_string(),
            site: SRC.find("doubled").unwrap_or(0),
        });
        ring.push(LomEvent::Failure {
            message: "you divided by zero".to_string(),
        });
        let report = with_concept_link(
            &failure_report(&ring, "lagom program", SRC),
            "you divided by zero",
        );
        assert!(report.contains("`double` with `x` = 0"), "{report}");
        assert!(report.contains("`doubled` became 0 (line"), "{report}");
        assert!(report.contains("it failed with: you divided by zero"), "{report}");
    }

    /// 26.5's answer 8: the concept footer names the lesson the failure
    /// teaches — and never fires for messages no lesson matches (an honest
    /// report teaches only what it knows).
    #[test]
    fn known_failures_link_their_concept_and_unknown_ones_stay_honest() {
        let hit = with_concept_link("— what happened —\n", "index 5 is past the end of this list (2 items).");
        assert!(hit.contains("— the concept —"), "{hit}");
        assert!(hit.contains("counts from 0"), "{hit}");
        let quiet = with_concept_link("— what happened —\n", "something no lesson covers");
        assert!(!quiet.contains("the concept"), "{quiet}");
    }

    #[test]
    fn empty_ring_report_is_honest_about_silence() {
        let report = failure_report(&dev_ring(), "program", SRC);
        assert!(report.contains("no recorded events"), "{report}");
    }
}
