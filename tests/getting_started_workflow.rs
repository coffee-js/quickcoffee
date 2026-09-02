use std::{path::PathBuf, process::Command};

fn repository(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path)
}

#[test]
fn readme_getting_started_commands_run_and_test_the_packaged_task() {
    let module_root = repository("examples/getting-started");
    let run = Command::new(env!("CARGO_BIN_EXE_qcoffee"))
        .args(["--json", "--module-root"])
        .arg(&module_root)
        .args([
            "demo",
            "--",
            r#"{"name":"  Fix login  ","tags":[" bug ","urgent"]}"#,
        ])
        .output()
        .expect("qcoffee starts");
    assert!(run.status.success());
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        concat!(
            "{\"ok\":true,\"exports\":{\"result\":{\"name\":\"Fix login\",",
            "\"tags\":[\"bug\",\"urgent\"]}}}\n"
        )
    );
    assert!(run.stderr.is_empty());

    let test = Command::new(env!("CARGO_BIN_EXE_qtest"))
        .args(["--module-root"])
        .arg(module_root)
        .arg("test")
        .output()
        .expect("qtest starts");
    assert!(test.status.success());
    assert_eq!(
        String::from_utf8_lossy(&test.stdout),
        "ok test/normalize_task.coffee\n"
    );
    assert!(test.stderr.is_empty());
}

#[test]
fn readme_leads_first_use_to_a_verified_release_archive() {
    let readme = std::fs::read_to_string(repository("README.md")).expect("README is readable");

    for expected in [
        "https://github.com/coffee-js/quickcoffee/releases/download/v${VERSION}",
        "SHA256SUMS",
        "aarch64-apple-darwin",
        "x86_64-apple-darwin",
        "x86_64-unknown-linux-gnu",
        "x86_64-pc-windows-msvc.zip",
        "./qcoffee --json --module-root examples/getting-started demo",
        "./qtest --module-root examples/getting-started test",
        "从源码构建 / Build from source",
    ] {
        assert!(readme.contains(expected), "README must include {expected}");
    }

    assert!(
        !readme.contains("cargo install --path ."),
        "a local source install must not be the first-use path"
    );
}
