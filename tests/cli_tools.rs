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
#[test]
fn qdocco_renders_escaped_source_and_checks() {
    let temp = std::env::temp_dir().join(format!("qcoffee-{}", std::process::id()));
    fs::create_dir_all(&temp).unwrap();
    let input = temp.join("demo.qc");
    let output = temp.join("demo.html");
    fs::write(&input, "## <Guide>\n1 + 2\n").unwrap();
    assert!(
        Command::new(bin("qdocco"))
            .args([input.to_str().unwrap(), "-o", output.to_str().unwrap()])
            .status()
            .unwrap()
            .success()
    );
    let page = fs::read_to_string(&output).unwrap();
    assert!(page.contains("&lt;Guide&gt;"));
    assert!(page.contains("Final value: <code>3</code>"));
    let non_test_document = Command::new(bin("qdocco"))
        .args(["--check", input.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(non_test_document.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&non_test_document.stderr).contains("expected true"));
    let check_input = temp.join("check.qc");
    fs::write(&check_input, "true\n").unwrap();
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
    assert!(document.contains("## Notes\n\n<Guide>"));
    assert!(document.contains("````quickcoffee\n1 + 2\n````"));
    assert!(document.contains("## Final value\n\n`3`"));
    let fenced_input = temp.join("fenced.qc");
    let fenced_output = temp.join("fenced.md");
    fs::write(&fenced_input, "## Fence\n# ````\ntrue\n").unwrap();
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
    assert!(fenced_document.contains("`````quickcoffee\n# ````\ntrue\n`````"));
    let block_input = temp.join("block-comment.qc");
    let block_output = temp.join("block-comment.html");
    fs::write(&block_input, "### hidden code ###\ntrue\n").unwrap();
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
    assert!(block_markdown_document.contains("````quickcoffee\n### hidden code ###\ntrue\n````"));
    let overwrite = Command::new(bin("qdocco"))
        .args([input.to_str().unwrap(), "-o", input.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(overwrite.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&overwrite.stderr).contains("output path must differ"));
    assert_eq!(fs::read_to_string(&input).unwrap(), "## <Guide>\n1 + 2\n");
    let conflict = Command::new(bin("qdocco"))
        .args(["--check", "--markdown", input.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(conflict.status.code(), Some(2));
    let _ = fs::remove_dir_all(temp);
}
#[test]
fn qtest_reports_success_and_failure() {
    let ok = Command::new(bin("qtest"))
        .arg("tests/scripts/arithmetic.qc")
        .output()
        .unwrap();
    assert!(ok.status.success());
    let stats = Command::new(bin("qtest"))
        .args(["--stats", "tests/scripts/arithmetic.qc"])
        .output()
        .unwrap();
    assert!(stats.status.success());
    assert!(String::from_utf8_lossy(&stats.stdout).contains("ok "));
    let stats_stderr = String::from_utf8_lossy(&stats.stderr);
    assert!(stats_stderr.contains("qtest stats:"));
    assert!(stats_stderr.contains("instructions="));
    assert!(stats_stderr.contains("fuel_remaining="));
    let invalid =
        std::env::temp_dir().join(format!("qcoffee-qtest-invalid-{}.qc", std::process::id()));
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
        "tests/scripts/arithmetic.qc",
        "tests/scripts/collections.qc",
        "tests/scripts/comprehension.qc",
        "tests/scripts/control-flow.qc",
        "tests/scripts/function.qc",
        "tests/scripts/stdlib.qc",
    ] {
        assert!(
            directory_stdout.contains(fixture),
            "qtest skipped {fixture}"
        );
    }
    let filtered = Command::new(bin("qtest"))
        .args(["--filter", "stdlib", "tests/scripts"])
        .output()
        .unwrap();
    assert!(filtered.status.success());
    assert_eq!(String::from_utf8_lossy(&filtered.stdout).lines().count(), 1);
    assert!(String::from_utf8_lossy(&filtered.stdout).contains("stdlib.qc"));
    let single_file = Command::new(bin("qtest"))
        .args(["--filter", "arithmetic.qc", "tests/scripts/arithmetic.qc"])
        .output()
        .unwrap();
    assert!(single_file.status.success());
    assert_eq!(
        String::from_utf8_lossy(&single_file.stdout).lines().count(),
        1
    );
    assert!(String::from_utf8_lossy(&single_file.stdout).contains("arithmetic.qc"));
    let listed = Command::new(bin("qtest"))
        .args(["--list", "--filter", "stdlib", "tests/scripts"])
        .output()
        .unwrap();
    assert!(listed.status.success());
    assert_eq!(
        String::from_utf8_lossy(&listed.stdout),
        "tests/scripts/stdlib.qc\n"
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
        .arg("tests/fixtures/failure.qc")
        .output()
        .unwrap();
    assert!(!bad.status.success());
    let temp = std::env::temp_dir().join(format!("qcoffee-qtest-fuel-{}.qc", std::process::id()));
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

#[cfg(unix)]
#[test]
fn qtest_ignores_recursive_directory_symlinks() {
    use std::os::unix::fs::symlink;

    let temp = std::env::temp_dir().join(format!("qcoffee-qtest-cycle-{}", std::process::id()));
    fs::create_dir_all(&temp).unwrap();
    fs::write(temp.join("pass.qc"), "true\n").unwrap();
    symlink(&temp, temp.join("loop")).unwrap();
    let output = Command::new(bin("qtest"))
        .args(["--tap", temp.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("TAP version 13\nok 1 - {}/pass.qc\n1..1\n", temp.display())
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
    let actual = temp.join("actual.qc");
    let alias = temp.join("alias.qc");
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
        .args(["--json", "tests/scripts/arithmetic.qc"])
        .output()
        .unwrap();
    assert!(ok.status.success());
    let stdout = String::from_utf8_lossy(&ok.stdout);
    assert_eq!(stdout.lines().count(), 1);
    assert!(stdout.ends_with("}\n"));
    assert!(stdout.contains("\"ok\":true"));
    assert!(stdout.contains("\"file\":\"tests/scripts/arithmetic.qc\""));
    let bad = Command::new(bin("qtest"))
        .args(["--json", "tests/fixtures/failure.qc"])
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
    fs::write(temp.join("a-fail.qc"), "1\n").unwrap();
    fs::write(temp.join("b-pass.qc"), "true\n").unwrap();
    let output = Command::new(bin("qtest"))
        .args(["--tap", temp.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let fail_path = temp.join("a-fail.qc");
    let pass_path = temp.join("b-pass.qc");
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
            temp.join("b-pass.qc").to_str().unwrap(),
            temp.join("a-fail.qc").to_str().unwrap(),
            temp.join("a-fail.qc").to_str().unwrap(),
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

    let temp = std::env::temp_dir().join(format!("qcoffee-dump-{}.qc", std::process::id()));
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
    let _ = fs::remove_file(temp);
}

#[test]
fn qcoffee_json_file_errors_include_the_opaque_input_path() {
    let temp = std::env::temp_dir().join(format!("qcoffee-named-source-{}.qc", std::process::id()));
    fs::write(&temp, "value = 1\n@\n").unwrap();
    let path = temp.to_str().unwrap();
    let output = Command::new(bin("qcoffee"))
        .args(["--json", path])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(&format!("\"source\":\"{path}\"")));
    assert!(stdout.ends_with(",\"line\":2}\n"));
    assert!(output.stderr.is_empty());
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

    let function = Command::new(bin("qcoffee"))
        .args(["--json", "-e", "(x) -> x"])
        .output()
        .unwrap();
    assert!(function.status.success());
    assert_eq!(
        String::from_utf8_lossy(&function.stdout),
        "{\"ok\":true,\"value\":{\"$quickcoffee\":\"function\"}}\n"
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
        "{\"ok\":false,\"kind\":\"parse\",\"message\":\"unexpected character '@'\",\"line\":1}\n"
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
    assert!(resource_stdout.ends_with("\"line\":null}\n"));

    let missing = Command::new(bin("qcoffee"))
        .args(["--json", "qcoffee-file-that-does-not-exist.qc"])
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
        .args(["--check", "tests/scripts/arithmetic.qc", "--json"])
        .output()
        .unwrap();
    assert_eq!(reverse_conflict.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&reverse_conflict.stderr).contains("--json"));
}

#[test]
fn qcoffee_fingerprint_is_stable_non_executing_and_mutually_exclusive() {
    let temp = std::env::temp_dir().join(format!("qcoffee-fingerprint-{}.qc", std::process::id()));
    let other = std::env::temp_dir().join(format!(
        "qcoffee-fingerprint-other-{}.qc",
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
        "closures-and-ranges",
        "map-spread",
        "negative-indexing",
        "stepped-string-iteration",
        "signed-by-iteration",
        "postfix-loops",
        "array-slices",
        "existence-tests",
        "existential-assignment",
        "name-updates",
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
        ] {
            assert!(line.contains(field), "missing {field} in {line}");
        }
        assert!(line.contains(&format!("\"version\":\"{}\"", env!("CARGO_PKG_VERSION"))));
        assert!(line.contains("\"compile_mad_ns\":0"));
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
    assert!(text_stdout.contains("profile_value_allocations="));
    assert!(text_stdout.contains("profile_environment_allocations="));
    let repeated = Command::new(bin("qbench"))
        .args(["--json", "--iterations", "1", "--repeat", "3"])
        .output()
        .unwrap();
    assert!(repeated.status.success());
    assert!(
        String::from_utf8_lossy(&repeated.stdout)
            .lines()
            .all(|line| line.contains("\"repeat\":3"))
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
    assert_eq!(stdout.lines().count(), 2);
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
    assert!(stats_stderr.contains("unexpected character '@'"));

    let conflict = Command::new(bin("qcoffee"))
        .args(["--interactive", "-e", "1 + 1"])
        .output()
        .unwrap();
    assert_eq!(conflict.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&conflict.stderr).contains("cannot be combined"));
}
