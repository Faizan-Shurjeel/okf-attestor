use std::fs;
use std::path::{Path, PathBuf};

use okf_attestor_core::{ReasonCode, Verdict, Verifier};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../fixtures")
}

#[test]
fn fixture_bundle_exercises_all_three_verdicts() {
    let report = Verifier::new().unwrap().verify_bundle(fixtures()).unwrap();

    assert_eq!(report.results.len(), 3);
    let result = |id: &str| {
        report
            .results
            .iter()
            .find(|item| item.concept == id)
            .unwrap()
    };
    assert_eq!(result("reproduced").verdict, Verdict::Reproduced);
    assert_eq!(result("reproduced").reason_code, ReasonCode::OutputMatch);
    assert_eq!(result("diverged").verdict, Verdict::Diverged);
    assert_eq!(result("diverged").reason_code, ReasonCode::OutputMismatch);
    assert_eq!(result("unattestable").verdict, Verdict::Unattestable);
    assert_eq!(
        result("unattestable").reason_code,
        ReasonCode::UnsupportedRuntime
    );
    assert_eq!(report.exit_code(), 1);
}

#[test]
fn one_concept_can_be_selected() {
    let result = Verifier::new()
        .unwrap()
        .verify_concept(fixtures(), "reproduced")
        .unwrap();
    assert_eq!(result.verdict, Verdict::Reproduced);
}

#[test]
fn modules_with_any_import_fail_closed() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fs::create_dir(root.join("artifacts")).unwrap();
    fs::write(root.join("artifacts/input"), b"").unwrap();
    fs::write(root.join("artifacts/expected"), b"").unwrap();
    let wasm = wat::parse_str(
        r#"(module
            (import "wasi_snapshot_preview1" "random_get" (func (param i32 i32) (result i32)))
            (memory (export "memory") 1)
            (func (export "okf_attestor_alloc") (param i32) (result i32) i32.const 0)
            (func (export "okf_attestor_compute") (param i32 i32) (result i64) i64.const 0))"#,
    )
    .unwrap();
    fs::write(root.join("artifacts/importing.wasm"), wasm).unwrap();
    write_concept(root, "importing");

    let result = Verifier::new()
        .unwrap()
        .verify_concept(root, "importing")
        .unwrap();
    assert_eq!(result.verdict, Verdict::Unattestable);
    assert_eq!(result.reason_code, ReasonCode::ForbiddenImport);
}

fn write_concept(root: &Path, id: &str) {
    fs::write(
        root.join(format!("{id}.md")),
        r#"---
type: Attested Computation
runtime: okf:wasm@1
computation: artifacts/importing.wasm
okf_attestor:
  protocol: 1
  input: artifacts/input
  expected_output: artifacts/expected
---
"#,
    )
    .unwrap();
}
