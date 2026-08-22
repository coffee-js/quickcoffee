use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
};
fn bin(name: &str) -> String {
    std::env::var(format!("CARGO_BIN_EXE_{name}")).expect("Cargo supplies bin path")
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
    assert!(
        Command::new(bin("qdocco"))
            .args(["--check", input.to_str().unwrap()])
            .status()
            .unwrap()
            .success()
    );
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
#[test]
fn qtest_json_output_is_one_stable_record_per_file() {
    let ok = Command::new(bin("qtest"))
        .args(["--json", "tests/scripts/arithmetic.qc"])
        .output()
        .unwrap();
    assert!(ok.status.success());
    let stdout = String::from_utf8_lossy(&ok.stdout);
    assert!(stdout.contains("\"ok\":true"));
    assert!(stdout.contains("\"file\":\"tests/scripts/arithmetic.qc\""));
    let bad = Command::new(bin("qtest"))
        .args(["--json", "tests/fixtures/failure.qc"])
        .output()
        .unwrap();
    assert!(!bad.status.success());
    let bad_stdout = String::from_utf8_lossy(&bad.stdout);
    assert!(bad_stdout.contains("\"ok\":false"));
    assert!(bad_stdout.contains("\"error\":\""));
}
#[test]
fn qcoffee_evaluation_fuel_and_disassembly_match_the_cli_contract() {
    let version = Command::new(bin("qcoffee"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).starts_with("qcoffee "));

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

    let args = Command::new(bin("qcoffee"))
        .args(["-e", "len(argv)", "--", "one", "two"])
        .output()
        .unwrap();
    assert!(args.status.success());
    assert_eq!(String::from_utf8_lossy(&args.stdout), "2\n");

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
