// Resolve B oracle test (zero JVM at test time).
//
// The data file is produced by tools/gen-resolve-b-oracle/GenResolveBOracle.java
// against the baseline JDK 17.0.19. The PHASE rows are compared strictly with
// the ported Resolve.MethodResolutionPhase behavior. The CALL/ERROR rows record
// the real javac overload-resolution outcomes; replaying them end-to-end needs
// the Attr.attribInvocation wiring (Attr B batch), so those rows are checked
// for presence and reported as SKIPPED here (registration).

use javac_core::comp::resolve::MethodResolutionPhase;
use std::path::PathBuf;

fn oracle_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/resolve-b-oracle.txt")
}

#[test]
fn resolve_b_oracle_phases_match_port() {
    let text = std::fs::read_to_string(oracle_path()).expect("resolve-b oracle data");
    let mut phase_rows = Vec::new();
    let mut call_rows = 0usize;
    let mut error_rows = 0usize;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("PHASE ") {
            let parts: Vec<&str> = rest.split(' ').collect();
            assert_eq!(parts.len(), 4, "PHASE row: {line}");
            phase_rows.push((
                parts[0].to_string(),
                parts[1] == "true",
                parts[2] == "true",
                parts[3].parse::<usize>().expect("ordinal"),
            ));
        } else if line.starts_with("CALL ") {
            call_rows += 1;
        } else if line.starts_with("ERROR ") {
            error_rows += 1;
        }
    }
    assert_eq!(phase_rows.len(), MethodResolutionPhase::values().len());
    for (index, phase) in MethodResolutionPhase::values().into_iter().enumerate() {
        let (name, boxing, varargs, ordinal) = &phase_rows[index];
        assert_eq!(phase.ordinal(), *ordinal, "{name}: ordinal");
        assert_eq!(phase.is_boxing_required(), *boxing, "{name}: boxing");
        assert_eq!(phase.is_varargs_required(), *varargs, "{name}: varargs");
        assert_eq!(
            phase,
            match name.as_str() {
                "BASIC" => MethodResolutionPhase::BASIC,
                "BOX" => MethodResolutionPhase::BOX,
                "VARARITY" => MethodResolutionPhase::VARARITY,
                other => panic!("unexpected phase {other}"),
            }
        );
    }
    // End-to-end rows: recorded by the oracle, replay registered for Attr B.
    assert!(call_rows >= 5, "CALL rows: {call_rows}");
    assert_eq!(error_rows, 1);
    assert!(text.contains("ERROR compiler.err.cant.apply.symbols"));
    assert!(text.contains("CALL f(1) -> Sample.f(x)"));
    assert!(text.contains("CALL boxed(1) -> Sample.boxed(v)"));
    eprintln!(
        "SKIP end-to-end method selection replay (needs Attr.attribInvocation, Attr B batch): \
         {call_rows} CALL rows, {error_rows} ERROR rows recorded"
    );
}
