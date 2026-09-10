use std::{fs, path::PathBuf, process::Command};

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
fn starter_rule_regression_can_be_diagnosed_and_repaired() {
    let root = std::env::temp_dir().join(format!(
        "quickcoffee-starter-recovery-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    fs::create_dir(root.join("test")).unwrap();
    for file in ["demo.coffee", "task.coffee", "test/normalize_task.coffee"] {
        fs::copy(
            repository(&format!("examples/getting-started/{file}")),
            root.join(file),
        )
        .unwrap();
    }
    let run_tests = || {
        Command::new(env!("CARGO_BIN_EXE_qtest"))
            .arg("--module-root")
            .arg(&root)
            .arg("test")
            .output()
            .unwrap()
    };
    assert!(run_tests().status.success());
    let rule_path = root.join("task.coffee");
    let original = fs::read_to_string(&rule_path).unwrap();
    assert!(original.contains("tags: sort(tags)"));
    fs::write(
        &rule_path,
        original.replace("tags: sort(tags)", "tags: tags"),
    )
    .unwrap();
    let failed = run_tests();
    assert_eq!(failed.status.code(), Some(1));
    let failure = format!(
        "{}{}",
        String::from_utf8_lossy(&failed.stdout),
        String::from_utf8_lossy(&failed.stderr)
    );
    assert!(
        failure.contains("not ok test/normalize_task.coffee"),
        "{failure}"
    );
    assert!(failure.contains("export test was false, expected true"));

    // Inspect the same input as the test to explain the false result.
    let input = r#"{"name":"  Write docs  ","tags":["ux"," daily "]}"#;
    let run_demo = || {
        Command::new(env!("CARGO_BIN_EXE_qcoffee"))
            .args(["--json", "--module-root"])
            .arg(&root)
            .args(["demo", "--", input])
            .output()
            .unwrap()
    };
    let broken = run_demo();
    assert!(broken.status.success());
    assert!(String::from_utf8_lossy(&broken.stdout).contains(r#""tags":["ux","daily"]"#));
    fs::write(&rule_path, original).unwrap();
    assert!(run_tests().status.success());
    let repaired = run_demo();
    assert!(repaired.status.success());
    assert!(String::from_utf8_lossy(&repaired.stdout).contains(r#""tags":["daily","ux"]"#));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn starter_invalid_input_has_actionable_json_errors() {
    for (input, expected) in [
        ("{", "json.parse"),
        (r#"{"name":"Write docs","tags":[1]}"#, "tags[0]"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_qcoffee"))
            .args(["--json", "--module-root"])
            .arg(repository("examples/getting-started"))
            .args(["demo", "--", input])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stderr.is_empty());
        let error = String::from_utf8_lossy(&output.stdout);
        assert!(error.contains(r#""ok":false"#), "{error}");
        assert!(error.contains(expected), "{error}");
        assert!(error.contains(r#""source":"task.coffee""#), "{error}");
        if expected == "tags[0]" {
            assert!(error.contains("input.invalid"), "{error}");
            assert!(error.contains(r#""expected":"string""#), "{error}");
        }
    }
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
