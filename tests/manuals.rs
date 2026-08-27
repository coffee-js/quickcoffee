use quickcoffee::{Context, Value};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

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
        assert!(
            source
                .lines()
                .any(|line| !line.starts_with("    ") && line.contains('`')),
            "{name} should mark technical prose with Markdown inline code"
        );
        assert!(
            !source
                .lines()
                .any(|line| line.trim_start().starts_with("```")),
            "{name} must keep executable literate code indented, not fenced"
        );
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

#[test]
fn repository_publishes_markdown_without_generated_html() {
    let readme = include_str!("../README.md");
    let makefile = include_str!("../Makefile");
    let gitignore = include_str!("../.gitignore");

    for locale in ["zh-CN", "classical-zh", "en", "latin", "devanagari-sa"] {
        let markdown = format!("docs/manual.{locale}.md");
        let html = format!("docs/manual.{locale}.html");
        assert!(
            Path::new(&markdown).is_file(),
            "{markdown} should be published"
        );
        assert!(
            !Path::new(&html).exists(),
            "{html} is generated and must not be published"
        );
    }

    assert!(readme.contains("docs/manual.zh-CN.md"));
    assert!(readme.contains("docs/manual.en.md"));
    assert!(!readme.contains("docs/manual.zh-CN.html"));
    assert!(!readme.contains("docs/manual.en.html"));
    assert!(makefile.contains("docs-html: doc-check"));
    assert!(makefile.contains("target/manuals/manual.zh-CN.html"));
    assert!(gitignore.lines().any(|line| line == "/docs/manual.*.html"));
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
