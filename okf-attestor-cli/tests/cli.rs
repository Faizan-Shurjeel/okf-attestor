use std::path::PathBuf;

use assert_cmd::Command;
use predicates::prelude::*;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../fixtures")
}

#[test]
fn json_report_is_machine_readable_and_divergence_exits_one() {
    Command::cargo_bin("okf-attestor")
        .unwrap()
        .args(["verify", fixtures().to_str().unwrap(), "--format", "json"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("\"schema_version\": 1"))
        .stdout(predicate::str::contains("\"verdict\": \"reproduced\""))
        .stdout(predicate::str::contains("\"verdict\": \"diverged\""))
        .stdout(predicate::str::contains("\"verdict\": \"unattestable\""));
}

#[test]
fn concept_filter_controls_exit_status() {
    Command::cargo_bin("okf-attestor")
        .unwrap()
        .args([
            "verify",
            fixtures().to_str().unwrap(),
            "--concept",
            "reproduced",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("reproduced: Reproduced"));

    Command::cargo_bin("okf-attestor")
        .unwrap()
        .args([
            "verify",
            fixtures().to_str().unwrap(),
            "--concept",
            "unattestable",
        ])
        .assert()
        .code(2)
        .stdout(predicate::str::contains("unattestable: Unattestable"));
}
