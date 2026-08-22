use quickcoffee::{Context, Value};
use std::process::Command;

const MANUALS: &[&str] = &[
    include_str!("../manuals/manual.zh-CN.qc"),
    include_str!("../manuals/manual.classical-zh.qc"),
    include_str!("../manuals/manual.en.qc"),
    include_str!("../manuals/manual.latin.qc"),
    include_str!("../manuals/manual.devanagari-sa.qc"),
];

#[test]
fn every_literate_manual_is_an_executable_passing_example() {
    for source in MANUALS {
        assert!(matches!(Context::new().eval(source), Ok(Value::Bool(true))));
    }
}

#[test]
fn qdocco_checks_every_manual_source() {
    let qdocco = std::env::var("CARGO_BIN_EXE_qdocco").expect("Cargo supplies qdocco path");
    for locale in ["zh-CN", "classical-zh", "en", "latin", "devanagari-sa"] {
        assert!(
            Command::new(&qdocco)
                .args(["--check", &format!("manuals/manual.{locale}.qc")])
                .status()
                .expect("qdocco starts")
                .success()
        );
    }
}
