use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

use quickcoffee::parse_json;

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_qcson"))
}

fn repository(path: impl AsRef<Path>) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(path)
}

fn run(args: &[&str], stdin: Option<&str>) -> Output {
    let mut command = Command::new(bin());
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if stdin.is_some() {
        command.stdin(Stdio::piped());
    }
    let mut child = command.spawn().expect("qcson starts");
    if let Some(source) = stdin {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(source.as_bytes())
            .unwrap();
    }
    child.wait_with_output().expect("qcson exits")
}

fn assert_success(output: &Output, expected: &[u8]) {
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, expected);
    assert!(output.stderr.is_empty());
}

#[test]
fn corpus_conversion_metrics_match_in_both_directions() {
    let root = repository("tests/cson/accept");
    let mut fixtures = fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("cson"))
        .collect::<Vec<_>>();
    fixtures.sort();
    assert_eq!(fixtures.len(), 14, "the accepted CSON metric changed");

    for cson in fixtures {
        let json = cson.with_extension("json");
        let expected = fs::read(&json).unwrap();
        let from_cson = run(&["to-json", cson.to_str().unwrap()], None);
        assert_success(&from_cson, &expected);

        let to_cson = run(&["to-cson", json.to_str().unwrap()], None);
        assert!(
            to_cson.status.success(),
            "{}",
            String::from_utf8_lossy(&to_cson.stderr)
        );
        assert!(to_cson.stderr.is_empty());
        let canonical_cson = String::from_utf8(to_cson.stdout).unwrap();
        let round_trip = run(&["to-json", "-"], Some(&canonical_cson));
        assert_success(&round_trip, &expected);
    }
}

#[test]
fn file_and_stdin_entry_paths_are_byte_deterministic() {
    let cson_path = repository("tests/cson/accept/nested_map.cson");
    let json_path = cson_path.with_extension("json");
    let cson_source = fs::read_to_string(&cson_path).unwrap();
    let json_source = fs::read_to_string(&json_path).unwrap();
    let cases = [
        ("to-json", cson_path, cson_source),
        ("to-cson", json_path, json_source),
    ];

    let mut paths = 0;
    for (direction, path, source) in cases {
        for stdin in [false, true] {
            paths += 1;
            let mut expected = None;
            for _ in 0..3 {
                let output = if stdin {
                    run(&[direction, "-"], Some(&source))
                } else {
                    run(&[direction, path.to_str().unwrap()], None)
                };
                assert!(output.status.success());
                assert!(output.stderr.is_empty());
                if let Some(expected) = &expected {
                    assert_eq!(&output.stdout, expected);
                } else {
                    expected = Some(output.stdout);
                }
            }
        }
    }
    assert_eq!(paths, 4);
}

#[test]
fn diagnostic_matrix_has_stable_human_and_json_channels() {
    struct Case {
        name: &'static str,
        args: Vec<String>,
        stdin: Option<&'static str>,
        status: i32,
        code: &'static str,
    }

    let missing = repository("tests/cson/does-not-exist.cson");
    let arithmetic = repository("tests/cson/reject/arithmetic.cson");
    let cases = vec![
        Case {
            name: "cson syntax",
            args: vec!["to-json".into(), arithmetic.display().to_string()],
            stdin: None,
            status: 1,
            code: "E_CSON_EXPRESSION",
        },
        Case {
            name: "cson resource",
            args: vec![
                "--max-output-bytes".into(),
                "4".into(),
                "to-cson".into(),
                "-".into(),
            ],
            stdin: Some("null"),
            status: 1,
            code: "E_CSON_OUTPUT_LIMIT",
        },
        Case {
            name: "json syntax",
            args: vec!["to-cson".into(), "-".into()],
            stdin: Some("[1,]"),
            status: 1,
            code: "E_JSON_SYNTAX",
        },
        Case {
            name: "json resource",
            args: vec![
                "--max-output-bytes".into(),
                "4".into(),
                "to-json".into(),
                "-".into(),
            ],
            stdin: Some("null"),
            status: 1,
            code: "E_JSON_RESOURCE",
        },
        Case {
            name: "read io",
            args: vec!["to-json".into(), missing.display().to_string()],
            stdin: None,
            status: 1,
            code: "E_QCSON_READ",
        },
        Case {
            name: "usage",
            args: vec![],
            stdin: None,
            status: 2,
            code: "E_QCSON_USAGE",
        },
    ];
    assert_eq!(cases.len(), 6);

    for case in cases {
        let borrowed = case.args.iter().map(String::as_str).collect::<Vec<_>>();
        let human = run(&borrowed, case.stdin);
        assert_eq!(human.status.code(), Some(case.status), "{}", case.name);
        assert!(human.stdout.is_empty(), "{}", case.name);
        let human_error = String::from_utf8(human.stderr).unwrap();
        assert!(
            human_error.contains(case.code),
            "{}: {human_error}",
            case.name
        );
        assert!(!human_error.contains("qcson-diagnostic.v1"));

        let mut machine_args = vec!["--diagnostic-format".to_owned(), "json".to_owned()];
        machine_args.extend(case.args);
        let borrowed = machine_args.iter().map(String::as_str).collect::<Vec<_>>();
        let machine = run(&borrowed, case.stdin);
        assert_eq!(machine.status.code(), Some(case.status), "{}", case.name);
        assert!(machine.stdout.is_empty(), "{}", case.name);
        let machine_error = String::from_utf8(machine.stderr).unwrap();
        assert_eq!(machine_error.lines().count(), 1, "{}", case.name);
        assert!(
            parse_json(machine_error.trim_end()).is_ok(),
            "{}",
            case.name
        );
        assert!(machine_error.contains("\"schema\":\"quickcoffee.qcson-diagnostic.v1\""));
        assert!(machine_error.contains(&format!("\"code\":\"{}\"", case.code)));
    }
}

#[test]
fn cli_input_and_output_boundaries_are_exact_and_atomic() {
    let source = "null\n";
    for (option, expected_limit) in [
        ("--max-input-bytes", "cson_input_bytes"),
        ("--max-output-bytes", "json_output_bytes"),
    ] {
        let below = run(
            &["--diagnostic-format", "json", option, "4", "to-json", "-"],
            Some(source),
        );
        assert_eq!(below.status.code(), Some(1));
        assert!(below.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&below.stderr)
                .contains(&format!("\"limit\":\"{expected_limit}\""))
        );

        for boundary in ["5", "6"] {
            let output = run(&[option, boundary, "to-json", "-"], Some(source));
            assert_success(&output, b"null\n");
        }
    }
}
