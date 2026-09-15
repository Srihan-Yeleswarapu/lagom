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
    give back x plus x

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
            lagom_mir::MirItem::Function(f) | lagom_mir::MirItem::Main(f) | lagom_mir::MirItem::Test(f) => f
                .blocks
                .iter()
                .all(|b| b.instrs.iter().all(|ins| !lagom_mir::is_event(ins))),
            _ => true,
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
        let report = failure_report(&ring, "lagom program", SRC);
        assert!(report.contains("`double` with `x` = 0"), "{report}");
        assert!(report.contains("`doubled` became 0 (line"), "{report}");
        assert!(report.contains("it failed with: you divided by zero"), "{report}");
    }

    #[test]
    fn empty_ring_report_is_honest_about_silence() {
        let report = failure_report(&dev_ring(), "program", SRC);
        assert!(report.contains("no recorded events"), "{report}");
    }
}
