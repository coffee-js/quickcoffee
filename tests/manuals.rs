use quickcoffee::{Context, Value};
use std::{path::PathBuf, process::Command};

const MANUALS: &[(&str, &str)] = &[
    (
        "manual.zh-CN.litcoffee",
        include_str!("../manuals/manual.zh-CN.litcoffee"),
    ),
    (
        "manual.classical-zh.litcoffee",
        include_str!("../manuals/manual.classical-zh.litcoffee"),
    ),
    (
        "manual.en.litcoffee",
        include_str!("../manuals/manual.en.litcoffee"),
    ),
    (
        "manual.latin.litcoffee",
        include_str!("../manuals/manual.latin.litcoffee"),
    ),
    (
        "manual.devanagari-sa.litcoffee",
        include_str!("../manuals/manual.devanagari-sa.litcoffee"),
    ),
];

#[test]
fn every_literate_manual_is_an_executable_passing_example() {
    for (name, source) in MANUALS {
        assert!(matches!(
            Context::new().eval_named(name, source),
            Ok(Value::Bool(true))
        ));
    }
}

#[test]
fn qdocco_checks_every_manual_source() {
    let qdocco = binary("qdocco");
    for locale in ["zh-CN", "classical-zh", "en", "latin", "devanagari-sa"] {
        assert!(
            Command::new(&qdocco)
                .args(["--check", &format!("manuals/manual.{locale}.litcoffee")])
                .status()
                .expect("qdocco starts")
                .success()
        );
    }
}

fn binary(name: &str) -> String {
    if let Ok(path) = std::env::var(format!("CARGO_BIN_EXE_{name}")) {
        return path;
    }
    let test_binary = std::env::current_exe().expect("test binary path is available");
    let target_debug = test_binary
        .parent()
        .and_then(|deps| deps.parent())
        .expect("test binary lives below target/debug/deps");
    let mut candidate = PathBuf::from(target_debug);
    candidate.push(name);
    if cfg!(windows) {
        candidate.set_extension("exe");
    }
    assert!(
        candidate.is_file(),
        "Cargo binary path is unavailable: {candidate:?}"
    );
    candidate.to_string_lossy().into_owned()
}
