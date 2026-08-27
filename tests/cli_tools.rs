use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};
fn bin(name: &str) -> String {
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
fn json_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                escaped.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => escaped.push(character),
        }
    }
    escaped
}
#[test]
fn qdocco_renders_escaped_source_and_checks() {
    let temp = std::env::temp_dir().join(format!("qcoffee-{}", std::process::id()));
    fs::create_dir_all(&temp).unwrap();
    let input = temp.join("demo.litcoffee");
    let output = temp.join("demo.html");
    fs::write(
        &input,
        "# <Guide>\n\nCall `Context::eval` with `<source>`.\n\n    1 + 2\n",
    )
    .unwrap();
    assert!(
        Command::new(bin("qdocco"))
            .args([input.to_str().unwrap(), "-o", output.to_str().unwrap()])
            .status()
            .unwrap()
            .success()
    );
    let page = fs::read_to_string(&output).unwrap();
    assert!(page.contains("<meta name=\"generator\" content=\"quickcoffee.qdocco.html.v1\">"));
    assert!(page.contains("&lt;Guide&gt;"));
    assert!(page.contains("Call <code>Context::eval</code> with <code>&lt;source&gt;</code>."));
    assert!(page.contains("Final value: <code>3</code>"));
    let non_test_document = Command::new(bin("qdocco"))
        .args(["--check", input.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(non_test_document.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&non_test_document.stderr).contains("expected true"));
    let check_input = temp.join("check.litcoffee");
    fs::write(&check_input, "Passing document.\n\n    true\n").unwrap();
    assert!(
        Command::new(bin("qdocco"))
            .args(["--check", check_input.to_str().unwrap()])
            .status()
            .unwrap()
            .success()
    );
    let markdown = temp.join("demo.md");
    assert!(
        Command::new(bin("qdocco"))
            .args([
                "--markdown",
                input.to_str().unwrap(),
                "-o",
                markdown.to_str().unwrap(),
            ])
            .status()
            .unwrap()
            .success()
    );
    let document = fs::read_to_string(&markdown).unwrap();
    assert!(document.starts_with("<!-- quickcoffee.qdocco.markdown.v1 -->\n"));
    assert!(document.contains("## Notes\n\n# <Guide>\n\nCall `Context::eval` with `<source>`."));
    assert!(document.contains("````coffee\n1 + 2\n````"));
    assert!(document.contains("## Final value\n\n`3`"));
    let fenced_input = temp.join("fenced.litcoffee");
    let fenced_output = temp.join("fenced.md");
    fs::write(&fenced_input, "Fence\n\n    # ````\n    true\n").unwrap();
    assert!(
        Command::new(bin("qdocco"))
            .args([
                "--markdown",
                fenced_input.to_str().unwrap(),
                "-o",
                fenced_output.to_str().unwrap(),
            ])
            .status()
            .unwrap()
            .success()
    );
    let fenced_document = fs::read_to_string(&fenced_output).unwrap();
    assert!(fenced_document.contains("`````coffee\n# ````\ntrue\n`````"));
    let block_input = temp.join("block-comment.litcoffee");
    let block_output = temp.join("block-comment.html");
    fs::write(
        &block_input,
        "Code comments remain code.\n\n    ### hidden code ###\n    true\n",
    )
    .unwrap();
    assert!(
        Command::new(bin("qdocco"))
            .args([
                block_input.to_str().unwrap(),
                "-o",
                block_output.to_str().unwrap()
            ])
            .status()
            .unwrap()
            .success()
    );
    let block_document = fs::read_to_string(&block_output).unwrap();
    assert!(block_document.contains("<pre><code>### hidden code ###\ntrue\n</code></pre>"));
    let block_markdown = temp.join("block-comment.md");
    assert!(
        Command::new(bin("qdocco"))
            .args([
                "--markdown",
                block_input.to_str().unwrap(),
                "-o",
                block_markdown.to_str().unwrap(),
            ])
            .status()
            .unwrap()
            .success()
    );
    let block_markdown_document = fs::read_to_string(&block_markdown).unwrap();
    assert!(block_markdown_document.contains("````coffee\n### hidden code ###\ntrue\n````"));
    let overwrite = Command::new(bin("qdocco"))
        .args([input.to_str().unwrap(), "-o", input.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(overwrite.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&overwrite.stderr).contains("output path must differ"));
    assert_eq!(
        fs::read_to_string(&input).unwrap(),
        "# <Guide>\n\nCall `Context::eval` with `<source>`.\n\n    1 + 2\n"
    );
    let conflict = Command::new(bin("qdocco"))
        .args(["--check", "--markdown", input.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(conflict.status.code(), Some(2));
    let incremental_conflict = Command::new(bin("qdocco"))
        .args(["--check", "--incremental", input.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(incremental_conflict.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&incremental_conflict.stderr)
            .contains("--check cannot be combined")
    );
    let incremental_input = temp.join("incremental.litcoffee");
    let incremental_output = incremental_input.with_extension("md");
    fs::write(&incremental_input, "Incremental.\n\n    40 + 2\n").unwrap();
    let initial = Command::new(bin("qdocco"))
        .args([
            "--markdown",
            "--incremental",
            incremental_input.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(initial.status.success());
    assert!(String::from_utf8_lossy(&initial.stdout).starts_with("wrote "));
    assert!(incremental_output.is_file());
    let unchanged = Command::new(bin("qdocco"))
        .args([
            "--markdown",
            "--incremental",
            incremental_input.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(unchanged.status.success());
    assert!(String::from_utf8_lossy(&unchanged.stdout).starts_with("unchanged "));
    fs::write(&incremental_input, "Incremental.\n\n    40 + 3\n").unwrap();
    let changed = Command::new(bin("qdocco"))
        .args([
            "--markdown",
            "--incremental",
            incremental_input.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(changed.status.success());
    assert!(String::from_utf8_lossy(&changed.stdout).starts_with("wrote "));
    assert!(
        fs::read_to_string(&incremental_output)
            .unwrap()
            .contains("## Final value\n\n`43`")
    );
    let ordinary = temp.join("ordinary.coffee");
    fs::write(&ordinary, "true\n").unwrap();
    let rejected = Command::new(bin("qdocco")).arg(ordinary).output().unwrap();
    assert_eq!(rejected.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&rejected.stderr).contains("qdocco expects a .litcoffee document")
    );
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn qcoffee_executes_and_checks_litcoffee_files() {
    let input = std::env::temp_dir().join(format!(
        "qcoffee-cli-literate-{}.litcoffee",
        std::process::id()
    ));
    fs::write(
        &input,
        "# Executable document\n\n    answer = 40\n    answer + 2\n",
    )
    .unwrap();
    let run = Command::new(bin("qcoffee")).arg(&input).output().unwrap();
    assert!(run.status.success());
    assert_eq!(String::from_utf8_lossy(&run.stdout), "42\n");
    let check = Command::new(bin("qcoffee"))
        .args(["--check", input.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(check.status.success());
    assert!(check.stdout.is_empty());
    let json = Command::new(bin("qcoffee"))
        .args(["--json", input.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(json.status.success());
    assert_eq!(
        String::from_utf8_lossy(&json.stdout),
        "{\"ok\":true,\"value\":42}\n"
    );
    let dump = Command::new(bin("qcoffee"))
        .args(["--dump-bytecode", input.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(dump.status.success());
    assert!(String::from_utf8_lossy(&dump.stdout).contains("Return"));
    let fingerprint = Command::new(bin("qcoffee"))
        .args(["--fingerprint", input.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(fingerprint.status.success());
    assert_eq!(
        String::from_utf8_lossy(&fingerprint.stdout).trim().len(),
        16
    );
    let _ = fs::remove_file(input);
}
#[test]
fn qtest_reports_success_and_failure() {
    let ok = Command::new(bin("qtest"))
        .arg("tests/scripts/arithmetic.coffee")
        .output()
        .unwrap();
    assert!(ok.status.success());
    let stats = Command::new(bin("qtest"))
        .args(["--stats", "tests/scripts/arithmetic.coffee"])
        .output()
        .unwrap();
    assert!(stats.status.success());
    assert!(String::from_utf8_lossy(&stats.stdout).contains("ok "));
    let stats_stderr = String::from_utf8_lossy(&stats.stderr);
    assert!(stats_stderr.contains("qtest stats:"));
    assert!(stats_stderr.contains("instructions="));
    assert!(stats_stderr.contains("fuel_remaining="));
    let invalid = std::env::temp_dir().join(format!(
        "qcoffee-qtest-invalid-{}.coffee",
        std::process::id()
    ));
    fs::write(&invalid, "@\n").unwrap();
    let invalid_stats = Command::new(bin("qtest"))
        .args(["--stats", invalid.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!invalid_stats.status.success());
    assert!(
        String::from_utf8_lossy(&invalid_stats.stderr).contains("qtest stats:")
            && String::from_utf8_lossy(&invalid_stats.stderr)
                .contains("instructions=0 fuel_remaining=0")
    );
    let _ = fs::remove_file(&invalid);
    let directory = Command::new(bin("qtest"))
        .arg("tests/scripts")
        .output()
        .unwrap();
    assert!(directory.status.success());
    let directory_stdout = String::from_utf8_lossy(&directory.stdout);
    for fixture in [
        "arithmetic.coffee",
        "collections.coffee",
        "comprehension.coffee",
        "control-flow.coffee",
        "function.coffee",
        "stdlib.coffee",
    ] {
        let fixture = PathBuf::from("tests/scripts").join(fixture);
        assert!(
            directory_stdout.contains(&fixture.to_string_lossy().into_owned()),
            "qtest skipped {}",
            fixture.display()
        );
    }
    let literate = std::env::temp_dir().join(format!(
        "qcoffee-qtest-literate-{}.litcoffee",
        std::process::id()
    ));
    fs::write(&literate, "Executable test prose.\n\n    true\n").unwrap();
    let literate_output = Command::new(bin("qtest")).arg(&literate).output().unwrap();
    assert!(literate_output.status.success());
    let _ = fs::remove_file(literate);
    let filtered = Command::new(bin("qtest"))
        .args(["--filter", "stdlib", "tests/scripts"])
        .output()
        .unwrap();
    assert!(filtered.status.success());
    assert_eq!(String::from_utf8_lossy(&filtered.stdout).lines().count(), 1);
    assert!(String::from_utf8_lossy(&filtered.stdout).contains("stdlib.coffee"));
    let single_file = Command::new(bin("qtest"))
        .args([
            "--filter",
            "arithmetic.coffee",
            "tests/scripts/arithmetic.coffee",
        ])
        .output()
        .unwrap();
    assert!(single_file.status.success());
    assert_eq!(
        String::from_utf8_lossy(&single_file.stdout).lines().count(),
        1
    );
    assert!(String::from_utf8_lossy(&single_file.stdout).contains("arithmetic.coffee"));
    let listed = Command::new(bin("qtest"))
        .args(["--list", "--filter", "stdlib", "tests/scripts"])
        .output()
        .unwrap();
    assert!(listed.status.success());
    let listed_path = PathBuf::from("tests/scripts").join("stdlib.coffee");
    assert_eq!(
        String::from_utf8_lossy(&listed.stdout),
        format!("{}\n", listed_path.display())
    );
    let missing_filter = Command::new(bin("qtest"))
        .args(["--filter", "does-not-exist", "tests/scripts"])
        .output()
        .unwrap();
    assert_eq!(missing_filter.status.code(), Some(2));
    let list_conflict = Command::new(bin("qtest"))
        .args(["--list", "--json", "tests/scripts"])
        .output()
        .unwrap();
    assert_eq!(list_conflict.status.code(), Some(2));
    let bad = Command::new(bin("qtest"))
        .arg("tests/fixtures/failure.coffee")
        .output()
        .unwrap();
    assert!(!bad.status.success());
    let temp =
        std::env::temp_dir().join(format!("qcoffee-qtest-fuel-{}.coffee", std::process::id()));
    fs::write(&temp, "while true then 1\n").unwrap();
    let exhausted = Command::new(bin("qtest"))
        .args(["--fuel", "10", temp.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!exhausted.status.success());
    assert!(String::from_utf8_lossy(&exhausted.stderr).contains("fuel exhausted"));
    let exhausted_stats = Command::new(bin("qtest"))
        .args(["--stats", "--fuel", "10", temp.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!exhausted_stats.status.success());
    let exhausted_stats_stderr = String::from_utf8_lossy(&exhausted_stats.stderr);
    assert!(exhausted_stats_stderr.contains("qtest stats:"));
    assert!(exhausted_stats_stderr.contains("instructions=10 fuel_remaining=0"));
    let _ = fs::remove_file(temp);
}

#[test]
fn qtest_discovers_only_canonical_source_extensions() {
    let temp =
        std::env::temp_dir().join(format!("qcoffee-qtest-extensions-{}", std::process::id()));
    fs::create_dir_all(&temp).unwrap();
    fs::write(temp.join("ordinary.coffee"), "true\n").unwrap();
    fs::write(
        temp.join("document.litcoffee"),
        "Executable prose.\n\n    true\n",
    )
    .unwrap();
    fs::write(temp.join("legacy.qc"), "@\n").unwrap();

    let discovered = Command::new(bin("qtest")).arg(&temp).output().unwrap();
    assert!(discovered.status.success());
    let stdout = String::from_utf8_lossy(&discovered.stdout);
    assert!(stdout.contains("ordinary.coffee"));
    assert!(stdout.contains("document.litcoffee"));
    assert!(!stdout.contains("legacy.qc"));

    let explicit_legacy = Command::new(bin("qtest"))
        .arg(temp.join("legacy.qc"))
        .output()
        .unwrap();
    assert_eq!(explicit_legacy.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&explicit_legacy.stderr)
            .contains("expected a .coffee or .litcoffee source file")
    );
    let _ = fs::remove_dir_all(temp);
}

#[cfg(unix)]
#[test]
fn qtest_ignores_recursive_directory_symlinks() {
    use std::os::unix::fs::symlink;

    let temp = std::env::temp_dir().join(format!("qcoffee-qtest-cycle-{}", std::process::id()));
    fs::create_dir_all(&temp).unwrap();
    fs::write(temp.join("pass.coffee"), "true\n").unwrap();
    symlink(&temp, temp.join("loop")).unwrap();
    let output = Command::new(bin("qtest"))
        .args(["--tap", temp.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!(
            "TAP version 13\nok 1 - {}/pass.coffee\n1..1\n",
            temp.display()
        )
    );
    let _ = fs::remove_dir_all(temp);
}

#[cfg(unix)]
#[test]
fn qtest_executes_a_file_symlink_only_once() {
    use std::os::unix::fs::symlink;

    let temp =
        std::env::temp_dir().join(format!("qcoffee-qtest-file-alias-{}", std::process::id()));
    fs::create_dir_all(&temp).unwrap();
    let actual = temp.join("actual.coffee");
    let alias = temp.join("alias.coffee");
    fs::write(&actual, "true\n").unwrap();
    symlink(&actual, &alias).unwrap();
    let output = Command::new(bin("qtest"))
        .args(["--tap", actual.to_str().unwrap(), alias.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("TAP version 13\nok 1 - {}\n1..1\n", actual.display())
    );
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn every_cli_reports_the_same_package_version() {
    for name in ["qcoffee", "qtest", "qdocco", "qbench"] {
        let output = Command::new(bin(name)).arg("--version").output().unwrap();
        assert!(output.status.success(), "{name} --version failed");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            format!("{name} {}\n", env!("CARGO_PKG_VERSION"))
        );
        assert!(output.stderr.is_empty(), "{name} --version wrote stderr");
    }
}
#[test]
fn qtest_json_output_is_one_stable_record_per_file() {
    let ok = Command::new(bin("qtest"))
        .args(["--json", "tests/scripts/arithmetic.coffee"])
        .output()
        .unwrap();
    assert!(ok.status.success());
    let stdout = String::from_utf8_lossy(&ok.stdout);
    assert_eq!(stdout.lines().count(), 1);
    assert!(stdout.ends_with("}\n"));
    assert!(stdout.contains("\"ok\":true"));
    assert!(stdout.contains("\"file\":\"tests/scripts/arithmetic.coffee\""));
    let bad = Command::new(bin("qtest"))
        .args(["--json", "tests/fixtures/failure.coffee"])
        .output()
        .unwrap();
    assert!(!bad.status.success());
    let bad_stdout = String::from_utf8_lossy(&bad.stdout);
    assert_eq!(bad_stdout.lines().count(), 1);
    let bad_line = bad_stdout.trim_end_matches('\n');
    assert!(bad_line.starts_with('{') && bad_line.ends_with('}'));
    assert!(bad_stdout.contains("\"ok\":false"));
    assert!(bad_stdout.contains("\"error\":\""));
}
#[test]
fn qtest_tap_output_is_deterministic_and_describes_failures() {
    let temp = std::env::temp_dir().join(format!("qcoffee-qtest-tap-{}", std::process::id()));
    fs::create_dir_all(&temp).unwrap();
    fs::write(temp.join("a-fail.coffee"), "1\n").unwrap();
    fs::write(temp.join("b-pass.coffee"), "true\n").unwrap();
    let output = Command::new(bin("qtest"))
        .args(["--tap", temp.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let fail_path = temp.join("a-fail.coffee");
    let pass_path = temp.join("b-pass.coffee");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!(
            "TAP version 13\nnot ok 1 - {}\n# final value was 1, expected true\nok 2 - {}\n1..2\n",
            fail_path.display(),
            pass_path.display()
        )
    );
    let reversed = Command::new(bin("qtest"))
        .args([
            "--tap",
            temp.join("b-pass.coffee").to_str().unwrap(),
            temp.join("a-fail.coffee").to_str().unwrap(),
            temp.join("a-fail.coffee").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!reversed.status.success());
    assert_eq!(
        String::from_utf8_lossy(&reversed.stdout),
        format!(
            "TAP version 13\nnot ok 1 - {}\n# final value was 1, expected true\nok 2 - {}\n1..2\n",
            fail_path.display(),
            pass_path.display()
        )
    );
    let conflict = Command::new(bin("qtest"))
        .args(["--json", "--tap", temp.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(conflict.status.code(), Some(2));
    let _ = fs::remove_dir_all(temp);
}
#[test]
fn qcoffee_evaluation_fuel_and_disassembly_match_the_cli_contract() {
    let version = Command::new(bin("qcoffee"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).starts_with("qcoffee "));

    let quit = Command::new(bin("qcoffee")).arg("--quit").output().unwrap();
    assert!(quit.status.success());
    assert!(quit.stdout.is_empty());
    assert!(quit.stderr.is_empty());

    let quit_conflict = Command::new(bin("qcoffee"))
        .args(["--quit", "-e", "1"])
        .output()
        .unwrap();
    assert_eq!(quit_conflict.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&quit_conflict.stderr).contains("--quit cannot"));

    let prefixed_quit_conflict = Command::new(bin("qcoffee"))
        .args(["--fuel", "1", "--quit"])
        .output()
        .unwrap();
    assert_eq!(prefixed_quit_conflict.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&prefixed_quit_conflict.stderr).contains("--quit cannot"));

    let evaluation = Command::new(bin("qcoffee"))
        .args(["-e", "1 + 2"])
        .output()
        .unwrap();
    assert!(evaluation.status.success());
    assert_eq!(String::from_utf8_lossy(&evaluation.stdout), "3\n");

    let stats = Command::new(bin("qcoffee"))
        .args(["--stats", "-e", "1 + 2"])
        .output()
        .unwrap();
    assert!(stats.status.success());
    assert_eq!(String::from_utf8_lossy(&stats.stdout), "3\n");
    let stats_stderr = String::from_utf8_lossy(&stats.stderr);
    assert!(stats_stderr.contains("qcoffee stats: instructions="));
    assert!(stats_stderr.contains("fuel_remaining="));
    assert!(stats_stderr.contains("value_allocations="));
    assert!(stats_stderr.contains("environment_allocations="));
    assert!(stats_stderr.contains("managed_objects_allocated="));
    assert!(stats_stderr.contains("managed_bytes_allocated="));

    let args = Command::new(bin("qcoffee"))
        .args(["-e", "len(argv)", "--", "one", "two"])
        .output()
        .unwrap();
    assert!(args.status.success());
    assert_eq!(String::from_utf8_lossy(&args.stdout), "2\n");

    let quit_argument = Command::new(bin("qcoffee"))
        .args(["-e", "argv[0]", "--", "--quit"])
        .output()
        .unwrap();
    assert!(quit_argument.status.success());
    assert_eq!(String::from_utf8_lossy(&quit_argument.stdout), "--quit\n");

    let mut stdin = Command::new(bin("qcoffee"))
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    stdin.stdin.take().unwrap().write_all(b"40 + 2\n").unwrap();
    let stdin_output = stdin.wait_with_output().unwrap();
    assert!(stdin_output.status.success());
    assert_eq!(String::from_utf8_lossy(&stdin_output.stdout), "42\n");

    let mut stdin_dump = Command::new(bin("qcoffee"))
        .args(["--dump-bytecode", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    stdin_dump
        .stdin
        .take()
        .unwrap()
        .write_all(b"1 + 2\n")
        .unwrap();
    let stdin_dump_output = stdin_dump.wait_with_output().unwrap();
    assert!(stdin_dump_output.status.success());
    assert!(String::from_utf8_lossy(&stdin_dump_output.stdout).contains("Return"));

    let exhausted = Command::new(bin("qcoffee"))
        .args(["--fuel", "10", "-e", "while true then 1"])
        .output()
        .unwrap();
    assert!(!exhausted.status.success());
    assert!(String::from_utf8_lossy(&exhausted.stderr).contains("fuel exhausted"));

    let exhausted_stats = Command::new(bin("qcoffee"))
        .args(["--stats", "--fuel", "10", "-e", "while true then 1"])
        .output()
        .unwrap();
    assert!(!exhausted_stats.status.success());
    let exhausted_stderr = String::from_utf8_lossy(&exhausted_stats.stderr);
    assert!(exhausted_stderr.contains("fuel exhausted"));
    assert!(exhausted_stderr.contains("qcoffee stats: instructions=10 fuel_remaining=0"));

    let stats_check = Command::new(bin("qcoffee"))
        .args(["--stats", "--check", "-"])
        .output()
        .unwrap();
    assert_eq!(stats_check.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&stats_check.stderr).contains("execution-mode alternatives"));

    let temp = std::env::temp_dir().join(format!("qcoffee-dump-{}.coffee", std::process::id()));
    fs::write(&temp, "1 + 2\n").unwrap();
    let conflicting_source = Command::new(bin("qcoffee"))
        .args(["-e", "1", "--check", temp.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(conflicting_source.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&conflicting_source.stderr).contains("execution-mode alternatives")
    );
    let reverse_conflict = Command::new(bin("qcoffee"))
        .args(["--check", temp.to_str().unwrap(), "-e", "1"])
        .output()
        .unwrap();
    assert_eq!(reverse_conflict.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&reverse_conflict.stderr).contains("-e cannot"));

    let repeated_dump = Command::new(bin("qcoffee"))
        .args([
            "--dump-bytecode",
            temp.to_str().unwrap(),
            "--dump-bytecode",
            temp.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(repeated_dump.status.code(), Some(2));

    let dump = Command::new(bin("qcoffee"))
        .args(["--dump-bytecode", temp.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(dump.status.success());
    assert!(String::from_utf8_lossy(&dump.stdout).contains("Return"));

    let check = Command::new(bin("qcoffee"))
        .args(["--check", temp.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(check.status.success());
    assert!(check.stdout.is_empty());

    fs::write(&temp, "while true then 1\n").unwrap();
    let non_executing_check = Command::new(bin("qcoffee"))
        .args(["--fuel", "1", "--check", temp.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(non_executing_check.status.success());

    fs::write(&temp, "value = 1\n@\n").unwrap();
    let invalid_check = Command::new(bin("qcoffee"))
        .args(["--check", temp.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!invalid_check.status.success());
    assert!(String::from_utf8_lossy(&invalid_check.stderr).contains("line 2"));

    fs::write(&temp, "first = [1 2]\nsecond = [3 4]\n").unwrap();
    let multi_error_check = Command::new(bin("qcoffee"))
        .args(["--check", temp.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(multi_error_check.status.code(), Some(1));
    assert!(multi_error_check.stdout.is_empty());
    let multi_error_stderr = String::from_utf8_lossy(&multi_error_check.stderr);
    let first = multi_error_stderr
        .find("parse error (line 1):")
        .expect("first malformed statement is reported");
    let second = multi_error_stderr
        .find("parse error (line 2):")
        .expect("second malformed statement is reported");
    assert!(first < second, "parse errors stay in source order");
    let _ = fs::remove_file(temp);
}

#[test]
fn qcoffee_json_file_errors_include_the_opaque_input_path() {
    let temp = std::env::temp_dir().join(format!(
        "qcoffee-named-source-{}.coffee",
        std::process::id()
    ));
    fs::write(&temp, "value = 1\n@\n").unwrap();
    let path = temp.to_str().unwrap();
    let output = Command::new(bin("qcoffee"))
        .args(["--json", path])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json_path = json_escape(path);
    assert!(stdout.contains(&format!("\"source\":\"{json_path}\"")));
    assert!(stdout.ends_with(",\"line\":2}\n"));
    assert!(output.stderr.is_empty());

    fs::write(&temp, "value = 1\nvalue + 'x'\n").unwrap();
    let runtime = Command::new(bin("qcoffee"))
        .args(["--json", path])
        .output()
        .unwrap();
    assert!(!runtime.status.success());
    let stdout = String::from_utf8_lossy(&runtime.stdout);
    assert!(stdout.contains("\"kind\":\"runtime\""));
    assert!(stdout.contains(&format!("\"source\":\"{json_path}\"")));
    assert!(stdout.ends_with(",\"line\":2}\n"));
    assert!(runtime.stderr.is_empty());
    let _ = fs::remove_file(temp);
}

#[test]
fn qcoffee_json_reports_values_and_structured_errors() {
    let value = Command::new(bin("qcoffee"))
        .args(["--json", "-e", "{answer: 42, ok: true}"])
        .output()
        .unwrap();
    assert!(value.status.success());
    assert_eq!(
        String::from_utf8_lossy(&value.stdout),
        "{\"ok\":true,\"value\":{\"answer\":42,\"ok\":true}}\n"
    );
    assert!(value.stderr.is_empty());

    let integer = Command::new(bin("qcoffee"))
        .args(["--json", "-e", "9007199254740993n"])
        .output()
        .unwrap();
    assert!(integer.status.success());
    assert_eq!(
        String::from_utf8_lossy(&integer.stdout),
        "{\"ok\":true,\"value\":{\"$quickcoffee\":\"integer\",\"value\":\"9007199254740993\"}}\n"
    );

    let decimal = Command::new(bin("qcoffee"))
        .args(["--json", "-e", "1234567890.012300m"])
        .output()
        .unwrap();
    assert!(decimal.status.success());
    assert_eq!(
        String::from_utf8_lossy(&decimal.stdout),
        "{\"ok\":true,\"value\":{\"$quickcoffee\":\"decimal\",\"value\":\"1234567890.0123\"}}\n"
    );

    let function = Command::new(bin("qcoffee"))
        .args(["--json", "-e", "(x) -> x"])
        .output()
        .unwrap();
    assert!(function.status.success());
    assert_eq!(
        String::from_utf8_lossy(&function.stdout),
        "{\"ok\":true,\"value\":{\"$quickcoffee\":\"function\"}}\n"
    );

    let class = Command::new(bin("qcoffee"))
        .args(["--json", "-e", "class Empty\n  value: -> nil\nEmpty"])
        .output()
        .unwrap();
    assert!(class.status.success());
    assert_eq!(
        String::from_utf8_lossy(&class.stdout),
        "{\"ok\":true,\"value\":{\"$quickcoffee\":\"class\"}}\n"
    );

    let instance = Command::new(bin("qcoffee"))
        .args(["--json", "-e", "class Empty\n  value: -> nil\nnew Empty()"])
        .output()
        .unwrap();
    assert!(instance.status.success());
    assert_eq!(
        String::from_utf8_lossy(&instance.stdout),
        "{\"ok\":true,\"value\":{\"$quickcoffee\":\"instance\"}}\n"
    );

    let structured_error = Command::new(bin("qcoffee"))
        .args([
            "--json",
            "-e",
            "error('invoice.missing', 'missing', {id: 42})",
        ])
        .output()
        .unwrap();
    assert!(structured_error.status.success());
    assert_eq!(
        String::from_utf8_lossy(&structured_error.stdout),
        "{\"ok\":true,\"value\":{\"$quickcoffee\":\"error\",\"code\":\"invoice.missing\",\"message\":\"missing\",\"data\":{\"id\":42},\"cause\":null}}\n"
    );

    let uncaught_domain = Command::new(bin("qcoffee"))
        .args([
            "--json",
            "-e",
            "throw error('invoice.missing', 'missing', {id: 42})",
        ])
        .output()
        .unwrap();
    assert!(!uncaught_domain.status.success());
    assert_eq!(
        String::from_utf8_lossy(&uncaught_domain.stdout),
        "{\"ok\":false,\"kind\":\"runtime\",\"message\":\"missing\",\"code\":\"invoice.missing\",\"data\":{\"id\":42},\"cause\":null,\"line\":1}\n"
    );

    let nil = Command::new(bin("qcoffee"))
        .args(["--json", "-e", "nil"])
        .output()
        .unwrap();
    assert!(nil.status.success());
    assert_eq!(
        String::from_utf8_lossy(&nil.stdout),
        "{\"ok\":true,\"value\":null}\n"
    );

    let parse_error = Command::new(bin("qcoffee"))
        .args(["--json", "-e", "@"])
        .output()
        .unwrap();
    assert!(!parse_error.status.success());
    assert_eq!(
        String::from_utf8_lossy(&parse_error.stdout),
        "{\"ok\":false,\"kind\":\"parse\",\"message\":\"expected receiver member name after @\",\"line\":1}\n"
    );
    assert!(parse_error.stderr.is_empty());

    let resource_error = Command::new(bin("qcoffee"))
        .args(["--json", "--fuel", "10", "-e", "while true then 1"])
        .output()
        .unwrap();
    assert!(!resource_error.status.success());
    let resource_stdout = String::from_utf8_lossy(&resource_error.stdout);
    assert!(resource_stdout.starts_with("{\"ok\":false,\"kind\":\"resource\""));
    assert!(resource_stdout.contains("fuel exhausted"));
    assert!(resource_stdout.ends_with("\"line\":1}\n"));

    let missing = Command::new(bin("qcoffee"))
        .args(["--json", "qcoffee-file-that-does-not-exist.coffee"])
        .output()
        .unwrap();
    assert!(!missing.status.success());
    let missing_stdout = String::from_utf8_lossy(&missing.stdout);
    assert!(
        missing_stdout.starts_with(
            "{\"ok\":false,\"stage\":\"read\",\"kind\":\"io\",\"message\":\"read error:"
        )
    );
    assert!(missing_stdout.ends_with("\",\"line\":null}\n"));
    assert!(missing.stderr.is_empty());

    let reverse_conflict = Command::new(bin("qcoffee"))
        .args(["--check", "tests/scripts/arithmetic.coffee", "--json"])
        .output()
        .unwrap();
    assert_eq!(reverse_conflict.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&reverse_conflict.stderr).contains("--json"));
}

#[test]
fn qcoffee_fingerprint_is_stable_non_executing_and_mutually_exclusive() {
    let temp =
        std::env::temp_dir().join(format!("qcoffee-fingerprint-{}.coffee", std::process::id()));
    let other = std::env::temp_dir().join(format!(
        "qcoffee-fingerprint-other-{}.coffee",
        std::process::id()
    ));
    fs::write(&temp, "print('side effect')\n1 + 2\n").unwrap();
    fs::write(&other, "print('side effect')\n1 + 3\n").unwrap();
    let first = Command::new(bin("qcoffee"))
        .args(["--fingerprint", temp.to_str().unwrap()])
        .output()
        .unwrap();
    let second = Command::new(bin("qcoffee"))
        .args(["--fingerprint", temp.to_str().unwrap()])
        .output()
        .unwrap();
    let different = Command::new(bin("qcoffee"))
        .args(["--fingerprint", other.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(first.status.success() && second.status.success() && different.status.success());
    assert_eq!(first.stdout, second.stdout);
    assert_ne!(first.stdout, different.stdout);
    let fingerprint = String::from_utf8_lossy(&first.stdout);
    assert_eq!(fingerprint.trim().len(), 16);
    assert!(fingerprint.trim().chars().all(|c| c.is_ascii_hexdigit()));
    assert!(first.stderr.is_empty());
    assert!(
        !first
            .stdout
            .windows(b"side effect".len())
            .any(|w| w == b"side effect")
    );
    let conflict = Command::new(bin("qcoffee"))
        .args(["--fingerprint", "--check", temp.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(conflict.status.code(), Some(2));
    let _ = fs::remove_file(temp);
    let _ = fs::remove_file(other);
}

#[test]
fn qcoffee_fingerprints_restricted_module_graphs_without_execution() {
    let root =
        std::env::temp_dir().join(format!("qcoffee-module-fingerprint-{}", std::process::id()));
    let app = root.join("app");
    fs::create_dir_all(&app).unwrap();
    fs::write(
        app.join("main.coffee"),
        "import { value } from './dependency'\nprint('must not execute')\nexport result = value",
    )
    .unwrap();
    fs::write(app.join("dependency.coffee"), "export value = 42").unwrap();
    let root_text = root.to_str().unwrap();

    let first = Command::new(bin("qcoffee"))
        .args(["--fingerprint", "--module-root", root_text, "app/main"])
        .output()
        .unwrap();
    let reordered = Command::new(bin("qcoffee"))
        .args(["--module-root", root_text, "app/main", "--fingerprint"])
        .output()
        .unwrap();
    assert!(first.status.success() && reordered.status.success());
    assert_eq!(first.stdout, reordered.stdout);
    assert!(first.stderr.is_empty() && reordered.stderr.is_empty());
    let fingerprint = String::from_utf8_lossy(&first.stdout);
    assert_eq!(fingerprint.trim().len(), 16);
    assert!(fingerprint.trim().chars().all(|c| c.is_ascii_hexdigit()));
    assert!(!fingerprint.contains("must not execute"));

    fs::write(
        app.join("dependency.coffee"),
        "# source-only change\nexport value = 42",
    )
    .unwrap();
    let changed = Command::new(bin("qcoffee"))
        .args(["--fingerprint", "--module-root", root_text, "app/main"])
        .output()
        .unwrap();
    assert!(changed.status.success());
    assert_ne!(first.stdout, changed.stdout);

    for args in [
        vec![
            "--json",
            "--fingerprint",
            "--module-root",
            root_text,
            "app/main",
        ],
        vec![
            "--stats",
            "--fingerprint",
            "--module-root",
            root_text,
            "app/main",
        ],
    ] {
        let conflict = Command::new(bin("qcoffee")).args(args).output().unwrap();
        assert_eq!(conflict.status.code(), Some(2));
    }
    let missing_input = Command::new(bin("qcoffee"))
        .arg("--fingerprint")
        .output()
        .unwrap();
    assert_eq!(missing_input.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&missing_input.stderr)
            .contains("requires a file or --module-root ROOT ENTRY")
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn qcoffee_compile_limit_flags_bound_reads_bytecode_and_module_graphs() {
    let boundary = Command::new(bin("qcoffee"))
        .args(["--max-source-bytes", "4", "-e", "true"])
        .output()
        .unwrap();
    assert!(boundary.status.success());
    assert_eq!(String::from_utf8_lossy(&boundary.stdout), "true\n");

    let source_error = Command::new(bin("qcoffee"))
        .args(["--json", "--max-source-bytes", "3", "-e", "true"])
        .output()
        .unwrap();
    assert_eq!(source_error.status.code(), Some(1));
    let source_json = String::from_utf8_lossy(&source_error.stdout);
    assert!(source_json.contains("\"kind\":\"resource\""));
    assert!(source_json.contains("source exceeds configured UTF-8 byte limit of 3"));

    let bytecode_error = Command::new(bin("qcoffee"))
        .args(["--max-bytecode-instructions", "1", "-e", "true"])
        .output()
        .unwrap();
    assert_eq!(bytecode_error.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&bytecode_error.stderr)
            .contains("bytecode exceeds configured recursive instruction limit of 1")
    );

    let root =
        std::env::temp_dir().join(format!("qcoffee-cli-compile-limits-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let source_path = root.join("single.coffee");
    fs::write(&source_path, "true\n").unwrap();
    let read_error = Command::new(bin("qcoffee"))
        .args([
            "--json",
            "--max-source-bytes",
            "4",
            source_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(read_error.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&read_error.stdout),
        "{\"ok\":false,\"stage\":\"read\",\"kind\":\"resource\",\"limit\":\"source_bytes\",\"message\":\"source exceeds configured UTF-8 byte limit of 4\",\"line\":null}\n"
    );

    fs::write(
        root.join("main.coffee"),
        "import { value } from './dependency'\nexport value = value\n",
    )
    .unwrap();
    fs::write(root.join("dependency.coffee"), "export value = 42\n").unwrap();
    let graph_error = Command::new(bin("qcoffee"))
        .args([
            "--max-module-graph-modules",
            "1",
            "--module-root",
            root.to_str().unwrap(),
            "main",
        ])
        .output()
        .unwrap();
    assert_eq!(graph_error.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&graph_error.stderr)
            .contains("module graph exceeds configured unique module limit of 1")
    );

    let invalid = Command::new(bin("qcoffee"))
        .args(["--max-source-bytes", "invalid", "-e", "true"])
        .output()
        .unwrap();
    assert_eq!(invalid.status.code(), Some(2));
    let quit_conflict = Command::new(bin("qcoffee"))
        .args(["--max-source-bytes", "4", "--quit"])
        .output()
        .unwrap();
    assert_eq!(quit_conflict.status.code(), Some(2));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn qcoffee_modules_require_an_explicit_restricted_root() {
    let root = std::env::temp_dir().join(format!("qcoffee-cli-modules-{}", std::process::id()));
    let app = root.join("app");
    let lib = root.join("lib");
    fs::create_dir_all(&app).unwrap();
    fs::create_dir_all(&lib).unwrap();
    fs::write(lib.join("math.coffee"), "export answer = 21\n").unwrap();
    fs::write(
        app.join("main.coffee"),
        "import { answer } from '../lib/math'\nexport result = answer * 2\nexport argc = len(argv)\n",
    )
    .unwrap();

    let root_text = root.to_str().unwrap();
    let run = Command::new(bin("qcoffee"))
        .args(["--module-root", root_text, "app/main", "--", "one", "two"])
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(stdout.contains("result: 42"));
    assert!(stdout.contains("argc: 2"));
    assert!(run.stderr.is_empty());

    let json = Command::new(bin("qcoffee"))
        .args(["--json", "--module-root", root_text, "app/main"])
        .output()
        .unwrap();
    assert!(json.status.success());
    assert_eq!(
        String::from_utf8_lossy(&json.stdout),
        "{\"ok\":true,\"exports\":{\"argc\":0,\"result\":42}}\n"
    );
    assert!(json.stderr.is_empty());

    let stats = Command::new(bin("qcoffee"))
        .args(["--stats", "--module-root", root_text, "app/main"])
        .output()
        .unwrap();
    assert!(stats.status.success());
    assert!(String::from_utf8_lossy(&stats.stderr).contains("qcoffee stats: instructions="));

    let missing_entry = Command::new(bin("qcoffee"))
        .args(["--json", "--module-root", root_text, "app/missing"])
        .output()
        .unwrap();
    assert_eq!(missing_entry.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&missing_entry.stdout)
            .contains("module not found: app/missing.coffee")
    );

    let missing_root = root.join("missing-root");
    let missing_root = Command::new(bin("qcoffee"))
        .args([
            "--json",
            "--module-root",
            missing_root.to_str().unwrap(),
            "app/main",
        ])
        .output()
        .unwrap();
    assert_eq!(missing_root.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&missing_root.stdout).contains("module root is unavailable"));

    fs::write(lib.join("broken.coffee"), "@\n").unwrap();
    fs::write(
        app.join("broken.coffee"),
        "import { value } from '../lib/broken'\nexport value = value\n",
    )
    .unwrap();
    let broken = Command::new(bin("qcoffee"))
        .args(["--json", "--module-root", root_text, "app/broken"])
        .output()
        .unwrap();
    assert_eq!(broken.status.code(), Some(1));
    let broken_stdout = String::from_utf8_lossy(&broken.stdout);
    assert!(broken_stdout.contains("\"source\":\"lib/broken.coffee\""));
    assert!(broken_stdout.ends_with(",\"line\":1}\n"));

    fs::write(
        app.join("escape.coffee"),
        "import { value } from '../../outside'\nexport value = value\n",
    )
    .unwrap();
    let escape = Command::new(bin("qcoffee"))
        .args(["--module-root", root_text, "app/escape"])
        .output()
        .unwrap();
    assert_eq!(escape.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&escape.stderr).contains("module path escapes configured root")
    );

    fs::write(
        app.join("cycle-a.coffee"),
        "import { value } from './cycle-b'\nexport value = value\n",
    )
    .unwrap();
    fs::write(
        app.join("cycle-b.coffee"),
        "import { value } from './cycle-a'\nexport value = value\n",
    )
    .unwrap();
    let cycle = Command::new(bin("qcoffee"))
        .args(["--module-root", root_text, "app/cycle-a"])
        .output()
        .unwrap();
    assert_eq!(cycle.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&cycle.stderr).contains(
        "circular module dependency: app/cycle-a.coffee -> app/cycle-b.coffee -> app/cycle-a.coffee"
    ));

    fs::write(app.join("fuel.coffee"), "loop 1\n").unwrap();
    let fuel = Command::new(bin("qcoffee"))
        .args(["--fuel", "8", "--module-root", root_text, "app/fuel"])
        .output()
        .unwrap();
    assert_eq!(fuel.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&fuel.stderr).contains("fuel exhausted"));

    let no_entry = Command::new(bin("qcoffee"))
        .args(["--module-root", root_text])
        .output()
        .unwrap();
    assert_eq!(no_entry.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&no_entry.stderr).contains("requires an entry module"));

    for args in [
        vec!["--module-root", root_text, "-e", "1"],
        vec!["--module-root", root_text, "--check", "app/main.coffee"],
        vec!["--module-root", root_text, "--interactive", "app/main"],
        vec!["--module-root", root_text, "app/main", "app/escape"],
    ] {
        let conflict = Command::new(bin("qcoffee")).args(args).output().unwrap();
        assert_eq!(conflict.status.code(), Some(2));
    }

    let ordinary_module = Command::new(bin("qcoffee"))
        .arg(app.join("main.coffee"))
        .output()
        .unwrap();
    assert_eq!(ordinary_module.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&ordinary_module.stderr).contains("module"));

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let outside = root.with_extension("outside.coffee");
        fs::write(&outside, "export value = 1\n").unwrap();
        symlink(&outside, app.join("escape-link.coffee")).unwrap();
        let link = Command::new(bin("qcoffee"))
            .args(["--module-root", root_text, "app/escape-link"])
            .output()
            .unwrap();
        assert_eq!(link.status.code(), Some(1));
        assert!(
            String::from_utf8_lossy(&link.stderr).contains("module path escapes configured root")
        );
        let _ = fs::remove_file(outside);
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn qbench_json_is_guarded_and_machine_readable() {
    let json = Command::new(bin("qbench"))
        .args(["--json", "--iterations", "1"])
        .output()
        .unwrap();
    assert!(json.status.success());
    let stdout = String::from_utf8_lossy(&json.stdout);
    let lines: Vec<_> = stdout.lines().collect();
    let expected_names = [
        "loop-core",
        "stdlib-abs",
        "stdlib-sum",
        "stdlib-min-max",
        "stdlib-range-sum",
        "stdlib-string-queries",
        "stdlib-stable-sort",
        "stdlib-concat",
        "stdlib-literal-replace",
        "closures-and-ranges",
        "call-containing-local-loop",
        "captured-local-loop",
        "map-spread",
        "member-lookup-loop",
        "class-construction-dispatch",
        "class-inherited-super-dispatch",
        "class-bound-callback",
        "negative-indexing",
        "stepped-string-iteration",
        "signed-by-iteration",
        "postfix-loops",
        "array-slices",
        "existence-tests",
        "existential-assignment",
        "name-updates",
        "exact-integer-updates",
        "exact-decimal-money",
        "json-exact-roundtrip",
        "floor-modulo",
        "bitwise",
        "multiline-strings",
        "string-iteration",
        "string-escapes",
        "string-indexing",
        "multiline-collections",
        "indented-maps",
        "implicit-calls",
        "execution-stats",
        "constant-folding",
        "bare-lambda",
        "stepped-iteration",
        "for-collection",
        "postfix-comprehension",
        "for-pattern-bindings",
        "maps-and-control",
        "soak-access",
        "nested-destructuring",
        "destructuring-rest",
        "chained-comparisons",
        "destructuring-parameters",
        "return-cleanup",
    ];
    assert_eq!(lines.len(), expected_names.len());
    for name in expected_names {
        assert_eq!(stdout.matches(&format!("\"name\":\"{name}\"")).count(), 1);
    }
    for line in lines {
        assert!(line.starts_with('{') && line.ends_with('}'));
        for field in [
            "\"schema\":\"quickcoffee.qbench.v1\"",
            "\"name\":\"",
            "\"iterations\":1",
            "\"repeat\":1",
            "\"expected\":\"",
            "\"compile_ns\":",
            "\"compile_mad_ns\":",
            "\"prepare_ns\":",
            "\"prepare_mad_ns\":",
            "\"verify_ns\":",
            "\"verify_mad_ns\":",
            "\"execute_ns\":",
            "\"execute_mad_ns\":",
            "\"profile_instructions\":",
            "\"profile_call_depth_peak\":",
            "\"profile_name_loads\":",
            "\"profile_name_stores\":",
            "\"profile_calls\":",
            "\"profile_container_ops\":",
            "\"profile_iterator_ops\":",
            "\"profile_exception_ops\":",
            "\"profile_value_allocations\":",
            "\"profile_environment_allocations\":",
            "\"profile_managed_objects_allocated\":",
            "\"profile_managed_bytes_allocated\":",
        ] {
            assert!(line.contains(field), "missing {field} in {line}");
        }
        assert!(line.contains(&format!("\"version\":\"{}\"", env!("CARGO_PKG_VERSION"))));
        assert!(line.contains("\"compile_mad_ns\":0"));
        assert!(line.contains("\"prepare_mad_ns\":0"));
        assert!(line.contains("\"verify_mad_ns\":0"));
        assert!(line.contains("\"execute_mad_ns\":0"));
    }
    let text = Command::new(bin("qbench"))
        .args(["--iterations", "1"])
        .output()
        .unwrap();
    assert!(text.status.success());
    let text_stdout = String::from_utf8_lossy(&text.stdout);
    assert!(!text_stdout.starts_with('{'));
    assert!(text_stdout.contains("schema=quickcoffee.qbench.v1"));
    assert!(text_stdout.contains(&format!("version={}", env!("CARGO_PKG_VERSION"))));
    assert!(text_stdout.contains("repeat=1"));
    assert!(text_stdout.contains("prepare_ns="));
    assert!(text_stdout.contains("prepare_mad_ns=0"));
    assert!(text_stdout.contains("profile_value_allocations="));
    assert!(text_stdout.contains("profile_environment_allocations="));
    assert!(text_stdout.contains("profile_managed_objects_allocated="));
    assert!(text_stdout.contains("profile_managed_bytes_allocated="));
    let repeated = Command::new(bin("qbench"))
        .args(["--json", "--iterations", "1", "--repeat", "3"])
        .output()
        .unwrap();
    assert!(repeated.status.success());
    assert!(
        String::from_utf8_lossy(&repeated.stdout)
            .lines()
            .all(|line| {
                line.contains("\"repeat\":3")
                    && line.contains("\"prepare_ns\":")
                    && line.contains("\"prepare_mad_ns\":")
            })
    );
    let invalid = Command::new(bin("qbench"))
        .args(["--iterations", "0"])
        .output()
        .unwrap();
    assert_eq!(invalid.status.code(), Some(2));
    let invalid_repeat = Command::new(bin("qbench"))
        .args(["--repeat", "0"])
        .output()
        .unwrap();
    assert_eq!(invalid_repeat.status.code(), Some(2));
    let invalid_compare_iterations = Command::new(bin("qbench"))
        .args(["--compare-qjs", "qjs", "--compare-iterations", "0"])
        .output()
        .unwrap();
    assert_eq!(invalid_compare_iterations.status.code(), Some(2));
    let listed = Command::new(bin("qbench")).arg("--list").output().unwrap();
    assert!(listed.status.success());
    assert_eq!(
        String::from_utf8_lossy(&listed.stdout)
            .lines()
            .collect::<Vec<_>>(),
        expected_names
    );
    assert!(listed.stderr.is_empty());
    let selected = Command::new(bin("qbench"))
        .args(["--only", "map-spread", "--json", "--iterations", "1"])
        .output()
        .unwrap();
    assert!(selected.status.success());
    let selected_stdout = String::from_utf8_lossy(&selected.stdout);
    assert_eq!(selected_stdout.lines().count(), 1);
    assert!(selected_stdout.contains("\"name\":\"map-spread\""));
    let replacement = Command::new(bin("qbench"))
        .args([
            "--only",
            "stdlib-literal-replace",
            "--json",
            "--iterations",
            "1",
        ])
        .output()
        .unwrap();
    assert!(replacement.status.success());
    let replacement_stdout = String::from_utf8_lossy(&replacement.stdout);
    assert_eq!(replacement_stdout.lines().count(), 1);
    assert!(replacement_stdout.contains("\"name\":\"stdlib-literal-replace\""));
    assert!(replacement_stdout.contains("\"profile_calls\":1,\"profile_container_ops\":0"));
    assert!(replacement_stdout.contains("\"profile_value_allocations\":1"));
    assert!(replacement_stdout.contains("\"profile_environment_allocations\":0"));
    assert!(replacement_stdout.contains("\"profile_managed_objects_allocated\":1"));
    assert!(replacement_stdout.contains("\"profile_managed_bytes_allocated\":22"));
    let member = Command::new(bin("qbench"))
        .args([
            "--only",
            "member-lookup-loop",
            "--json",
            "--iterations",
            "1",
        ])
        .output()
        .unwrap();
    assert!(member.status.success());
    let member_stdout = String::from_utf8_lossy(&member.stdout);
    assert_eq!(member_stdout.lines().count(), 1);
    assert!(member_stdout.contains("\"name\":\"member-lookup-loop\""));
    assert!(member_stdout.contains("\"expected\":\"1000\",\"compile_ns\":"));
    assert!(member_stdout.contains("\"profile_container_ops\":400,\"profile_iterator_ops\":"));
    let classes = Command::new(bin("qbench"))
        .args([
            "--only",
            "class-construction-dispatch",
            "--json",
            "--iterations",
            "1",
        ])
        .output()
        .unwrap();
    assert!(classes.status.success());
    let classes_stdout = String::from_utf8_lossy(&classes.stdout);
    assert_eq!(classes_stdout.lines().count(), 1);
    assert!(classes_stdout.contains("\"name\":\"class-construction-dispatch\""));
    assert!(classes_stdout.contains("\"expected\":\"5050\",\"compile_ns\":"));
    assert!(classes_stdout.contains("\"profile_calls\":200,\"profile_container_ops\":301"));
    assert!(classes_stdout.contains("\"profile_value_allocations\":103"));
    assert!(classes_stdout.contains("\"profile_environment_allocations\":200"));
    let inherited = Command::new(bin("qbench"))
        .args([
            "--only",
            "class-inherited-super-dispatch",
            "--json",
            "--iterations",
            "1",
        ])
        .output()
        .unwrap();
    assert!(inherited.status.success());
    let inherited_stdout = String::from_utf8_lossy(&inherited.stdout);
    assert_eq!(inherited_stdout.lines().count(), 1);
    assert!(inherited_stdout.contains("\"name\":\"class-inherited-super-dispatch\""));
    assert!(inherited_stdout.contains("\"expected\":\"4200\",\"compile_ns\":"));
    assert!(inherited_stdout.contains("\"profile_calls\":201,\"profile_container_ops\":103"));
    assert!(inherited_stdout.contains("\"profile_value_allocations\":6"));
    assert!(inherited_stdout.contains("\"profile_environment_allocations\":201"));
    let bound = Command::new(bin("qbench"))
        .args([
            "--only",
            "class-bound-callback",
            "--json",
            "--iterations",
            "1",
        ])
        .output()
        .unwrap();
    assert!(bound.status.success());
    let bound_stdout = String::from_utf8_lossy(&bound.stdout);
    assert_eq!(bound_stdout.lines().count(), 1);
    assert!(bound_stdout.contains("\"name\":\"class-bound-callback\""));
    assert!(bound_stdout.contains("\"expected\":\"5050\",\"compile_ns\":"));
    assert!(bound_stdout.contains("\"profile_calls\":102,\"profile_container_ops\":302"));
    assert!(bound_stdout.contains("\"profile_value_allocations\":5"));
    assert!(bound_stdout.contains("\"profile_environment_allocations\":102"));
    let unknown = Command::new(bin("qbench"))
        .args(["--only", "missing-workload"])
        .output()
        .unwrap();
    assert_eq!(unknown.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("use --list"));
    let list_conflict = Command::new(bin("qbench"))
        .args(["--list", "--only", "map-spread"])
        .output()
        .unwrap();
    assert_eq!(list_conflict.status.code(), Some(2));
    let compare_conflict = Command::new(bin("qbench"))
        .args(["--list", "--compare-qjs", "qjs"])
        .output()
        .unwrap();
    assert_eq!(compare_conflict.status.code(), Some(2));
}

#[cfg(unix)]
#[test]
fn qcompare_separates_startup_compile_hot_and_cli_phases() {
    use std::os::unix::fs::PermissionsExt;

    let temp = std::env::temp_dir().join(format!("qcoffee-qjs-{}", std::process::id()));
    fs::create_dir_all(&temp).unwrap();
    let fake_qjs = temp.join("qjs");
    fs::write(
        &fake_qjs,
        r#"#!/bin/sh
if [ "$1" = "--quit" ]; then
  exit 0
fi
if [ "$1" = "--std" ]; then
  printf '100 200\n'
  exit 0
fi
case "$2" in
  *1000000*) printf '499999500000\n' ;;
  *250000*) printf '250000\n' ;;
  *array-build-index-iterate*) printf '1498500\n' ;;
  *map-own-lookup*) printf '100000\n' ;;
  *map-functional-update*) printf '507502\n' ;;
  *unicode-scalar-iterate*) printf '15000\n' ;;
  *unicode-scalar-index*) printf '22000\n' ;;
  *) exit 1 ;;
esac
"#,
    )
    .unwrap();
    let mut permissions = fs::metadata(&fake_qjs).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_qjs, permissions).unwrap();

    let output = Command::new(bin("qbench"))
        .args([
            "--compare-qjs",
            fake_qjs.to_str().unwrap(),
            "--compare-iterations",
            "1",
            "--repeat",
            "1",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "qbench failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected = [
        ("scalar-loop", "499999500000"),
        ("function-loop", "250000"),
        ("array-build-index-iterate", "1498500"),
        ("map-own-lookup", "100000"),
        ("map-functional-update", "507502"),
        ("unicode-scalar-iterate", "15000"),
        ("unicode-scalar-index", "22000"),
    ];
    assert_eq!(stdout.lines().count(), expected.len());
    for (name, value) in expected {
        let name_field = format!("\"name\":\"{name}\"");
        let lines = stdout
            .lines()
            .filter(|line| line.contains(&name_field))
            .collect::<Vec<_>>();
        assert_eq!(lines.len(), 1, "missing or duplicate workload {name}");
        assert!(
            lines[0].contains(&format!("\"expected\":\"{value}\"")),
            "workload {name} did not retain expected value {value}: {}",
            lines[0]
        );
    }
    for line in stdout.lines() {
        for field in [
            "\"schema\":\"quickcoffee.qcompare.v1\"",
            "\"quickcoffee_startup_ns\":",
            "\"quickcoffee_startup_mad_ns\":0",
            "\"quickjs_startup_ns\":",
            "\"quickjs_startup_mad_ns\":0",
            "\"quickcoffee_compile_ns\":",
            "\"quickcoffee_compile_mad_ns\":0",
            "\"quickjs_compile_ns\":100",
            "\"quickjs_compile_mad_ns\":0",
            "\"quickcoffee_hot_ns\":",
            "\"quickcoffee_hot_mad_ns\":0",
            "\"quickjs_hot_ns\":200",
            "\"quickjs_hot_mad_ns\":0",
            "\"quickcoffee_cli_ns\":",
            "\"quickcoffee_cli_mad_ns\":0",
            "\"quickjs_cli_ns\":",
            "\"quickjs_cli_mad_ns\":0",
        ] {
            assert!(line.contains(field), "missing {field} in {line}");
        }
    }
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn qcoffee_interactive_session_preserves_context_and_recovers_from_errors() {
    let mut process = Command::new(bin("qcoffee"))
        .arg("--interactive")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    process
        .stdin
        .take()
        .unwrap()
        .write_all(b"answer = 40\nanswer + 2\nmissing + 1\nanswer + 2\n:quit\n")
        .unwrap();
    let output = process.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "40\n42\n42\n");
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown name 'missing'"));

    let mut stats_process = Command::new(bin("qcoffee"))
        .args(["--interactive", "--stats"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    stats_process
        .stdin
        .take()
        .unwrap()
        .write_all(b"1 + 2\n@\n3 + 4\n:quit\n")
        .unwrap();
    let stats_output = stats_process.wait_with_output().unwrap();
    assert!(stats_output.status.success());
    assert_eq!(String::from_utf8_lossy(&stats_output.stdout), "3\n7\n");
    let stats_stderr = String::from_utf8_lossy(&stats_output.stderr);
    assert_eq!(stats_stderr.matches("qcoffee stats:").count(), 2);
    assert!(stats_stderr.contains("expected receiver member name after @"));

    let conflict = Command::new(bin("qcoffee"))
        .args(["--interactive", "-e", "1 + 1"])
        .output()
        .unwrap();
    assert_eq!(conflict.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&conflict.stderr).contains("cannot be combined"));
}
