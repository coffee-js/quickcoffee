use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use quickcoffee::{Context, Value};

const CORPUS_ROOT: &str = "tests/cson";
const REQUIRED_FEATURES: &[&str] = &[
    "arithmetic",
    "array",
    "braced-map",
    "call",
    "comma",
    "comment",
    "duplicate-key",
    "empty-container",
    "empty-input",
    "exact-number-extension",
    "function",
    "identifier-value",
    "indentation",
    "interpolation",
    "key",
    "literal",
    "multiline-string",
    "negative-zero",
    "nested-map",
    "newline",
    "nonfinite-number",
    "number",
    "regex",
    "root-map",
    "root-value",
    "string",
    "triple-double-string",
    "unicode",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Decision {
    Accept,
    Reject,
    Defer,
}

#[derive(Debug)]
struct Case<'a> {
    id: &'a str,
    decision: Decision,
    features: Vec<&'a str>,
    fixture: &'a str,
    expectation: &'a str,
}

fn load_cases() -> Vec<Case<'static>> {
    let matrix = include_str!("cson/matrix.tsv");
    let mut lines = matrix.lines();
    assert_eq!(
        lines.next(),
        Some("id\tdecision\tfeatures\tfixture\texpectation")
    );

    lines
        .filter(|line| !line.is_empty())
        .map(|line| {
            let columns = line.split('\t').collect::<Vec<_>>();
            assert_eq!(columns.len(), 5, "invalid matrix row: {line:?}");
            let decision = match columns[1] {
                "accept" => Decision::Accept,
                "reject" => Decision::Reject,
                "defer" => Decision::Defer,
                other => panic!("invalid CSON decision {other:?}"),
            };
            Case {
                id: columns[0],
                decision,
                features: columns[2].split(',').collect(),
                fixture: columns[3],
                expectation: columns[4],
            }
        })
        .collect()
}

fn corpus_path(relative: &str) -> PathBuf {
    assert!(!relative.is_empty());
    let relative = Path::new(relative);
    assert!(!relative.is_absolute());
    assert!(
        relative
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_))),
        "corpus path must not escape its root: {relative:?}"
    );
    Path::new(CORPUS_ROOT).join(relative)
}

fn canonical_json(source: &str) -> String {
    let source = source.trim_end_matches(['\r', '\n']);
    let mut context = Context::new();
    context.set_global("expected_json", Value::from(source));
    context
        .eval("encode_json(parse_json(expected_json))")
        .unwrap_or_else(|error| panic!("invalid expected JSON {source:?}: {error}"))
        .as_str()
        .expect("encode_json returns String")
        .to_owned()
}

fn assert_identifier(value: &str, separator: char) {
    assert!(!value.is_empty());
    assert!(value.bytes().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == separator as u8
    }));
}

#[test]
fn cson_matrix_is_complete_and_executable_before_the_parser_exists() {
    let cases = load_cases();
    assert!(cases.len() >= 20, "the compatibility surface is too small");

    let mut ids = BTreeSet::new();
    let mut features = BTreeSet::new();
    let mut referenced_files = BTreeSet::new();
    let mut decisions = BTreeSet::new();

    for case in &cases {
        assert_identifier(case.id, '_');
        assert!(ids.insert(case.id), "duplicate case id: {}", case.id);
        decisions.insert(match case.decision {
            Decision::Accept => "accept",
            Decision::Reject => "reject",
            Decision::Defer => "defer",
        });

        assert!(!case.features.is_empty());
        for feature in &case.features {
            assert_identifier(feature, '-');
            features.insert(*feature);
        }

        let decision_directory = match case.decision {
            Decision::Accept => "accept/",
            Decision::Reject => "reject/",
            Decision::Defer => "defer/",
        };
        assert!(case.fixture.starts_with(decision_directory));
        assert!(case.expectation.starts_with(decision_directory));
        assert!(case.fixture.ends_with(".cson"));
        let expected_suffix = match case.decision {
            Decision::Accept => ".json",
            Decision::Reject => ".error",
            Decision::Defer => ".defer",
        };
        assert!(case.expectation.ends_with(expected_suffix));

        let fixture_path = corpus_path(case.fixture);
        let expectation_path = corpus_path(case.expectation);
        let fixture = fs::read_to_string(&fixture_path)
            .unwrap_or_else(|error| panic!("cannot read {fixture_path:?}: {error}"));
        let expectation = fs::read_to_string(&expectation_path)
            .unwrap_or_else(|error| panic!("cannot read {expectation_path:?}: {error}"));
        assert!(!fixture.is_empty(), "empty fixture: {fixture_path:?}");
        assert!(!expectation.trim().is_empty());
        assert!(
            !fixture.starts_with('\u{feff}'),
            "BOM is not part of CSON v1"
        );

        if fixture.contains('\r') {
            assert!(
                fixture.as_bytes().windows(2).any(|pair| pair == b"\r\n"),
                "carriage returns must occur in CRLF pairs: {fixture_path:?}"
            );
            assert!(!fixture.replace("\r\n", "").contains('\r'));
        }

        match case.decision {
            Decision::Accept => {
                let normalized = expectation.trim_end_matches(['\r', '\n']);
                assert_eq!(canonical_json(&expectation), normalized);
            }
            Decision::Reject => {
                let code = expectation.trim();
                assert!(code.starts_with("E_CSON_"));
                assert!(code.bytes().all(|byte| byte.is_ascii_uppercase()
                    || byte.is_ascii_digit()
                    || byte == b'_'));
            }
            Decision::Defer => assert_identifier(expectation.trim(), '-'),
        }

        assert!(referenced_files.insert(case.fixture.to_owned()));
        assert!(referenced_files.insert(case.expectation.to_owned()));
    }

    assert_eq!(decisions, BTreeSet::from(["accept", "defer", "reject"]));
    for required in REQUIRED_FEATURES {
        assert!(
            features.contains(required),
            "missing CSON feature {required:?}"
        );
    }

    let mut actual_files = BTreeSet::new();
    for directory in ["accept", "reject", "defer"] {
        for entry in fs::read_dir(Path::new(CORPUS_ROOT).join(directory)).unwrap() {
            let path = entry.unwrap().path();
            assert!(path.is_file(), "unexpected corpus directory: {path:?}");
            actual_files.insert(
                path.strip_prefix(CORPUS_ROOT)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
    assert_eq!(actual_files, referenced_files);
}

#[test]
fn cson_fixtures_keep_the_native_github_data_extension() {
    let attributes = fs::read_to_string(".gitattributes").expect("attributes file exists");
    assert!(!attributes.lines().any(|line| {
        line.split_whitespace().next() == Some("*.cson") && line.contains("linguist-language")
    }));
    assert!(attributes.lines().any(|line| {
        line.split_whitespace().eq([
            "tests/cson/accept/crlf_tabs.cson",
            "-text",
            "!eol",
            "whitespace=cr-at-eol",
        ])
    }));

    let crlf = fs::read(corpus_path("accept/crlf_tabs.cson")).unwrap();
    assert!(crlf.windows(2).any(|pair| pair == b"\r\n"));
    assert!(crlf.contains(&b'\t'));
}
