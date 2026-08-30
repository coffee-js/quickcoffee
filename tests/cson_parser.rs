use std::panic::{AssertUnwindSafe, catch_unwind};

use quickcoffee::{CsonErrorCode, CsonLimits, ValueKind, parse_cson, parse_cson_with_limits};

fn error_code(source: &str, limits: CsonLimits) -> CsonErrorCode {
    parse_cson_with_limits(source, limits)
        .expect_err("source must exceed the tested boundary")
        .code()
}

#[test]
fn public_limits_default_getters_and_builders_are_stable() {
    let defaults = CsonLimits::default();
    assert_eq!(defaults.max_input_bytes(), 1_000_000);
    assert_eq!(defaults.max_output_bytes(), 1_000_000);
    assert_eq!(defaults.max_string_bytes(), 1_000_000);
    assert_eq!(defaults.max_values(), 100_000);
    assert_eq!(defaults.max_container_items(), 100_000);
    assert_eq!(defaults.max_nesting_depth(), 128);
    assert_eq!(defaults.max_integer_bits(), 1_000_000);
    assert_eq!(defaults.max_decimal_coefficient_bits(), 1_000_000);
    assert_eq!(defaults.max_decimal_scale(), 100_000);
    assert_eq!(defaults.max_work_units(), 4_000_000);
    assert_eq!(defaults.max_diagnostics(), 32);

    let tightened = defaults
        .with_max_input_bytes(1)
        .with_max_output_bytes(2)
        .with_max_string_bytes(3)
        .with_max_values(4)
        .with_max_container_items(5)
        .with_max_nesting_depth(6)
        .with_max_integer_bits(7)
        .with_max_decimal_coefficient_bits(8)
        .with_max_decimal_scale(9)
        .with_max_work_units(10)
        .with_max_diagnostics(11);
    assert_eq!(tightened.max_input_bytes(), 1);
    assert_eq!(tightened.max_output_bytes(), 2);
    assert_eq!(tightened.max_string_bytes(), 3);
    assert_eq!(tightened.max_values(), 4);
    assert_eq!(tightened.max_container_items(), 5);
    assert_eq!(tightened.max_nesting_depth(), 6);
    assert_eq!(tightened.max_integer_bits(), 7);
    assert_eq!(tightened.max_decimal_coefficient_bits(), 8);
    assert_eq!(tightened.max_decimal_scale(), 9);
    assert_eq!(tightened.max_work_units(), 10);
    assert_eq!(tightened.max_diagnostics(), 11);
}

#[test]
fn stable_error_codes_are_public_and_machine_readable() {
    let cases = [
        (CsonErrorCode::Syntax, "E_CSON_SYNTAX"),
        (CsonErrorCode::Indentation, "E_CSON_INDENTATION"),
        (CsonErrorCode::DuplicateKey, "E_CSON_DUPLICATE_KEY"),
        (CsonErrorCode::Interpolation, "E_CSON_INTERPOLATION"),
        (CsonErrorCode::Expression, "E_CSON_EXPRESSION"),
        (CsonErrorCode::IdentifierValue, "E_CSON_IDENTIFIER_VALUE"),
        (CsonErrorCode::Number, "E_CSON_NUMBER"),
        (CsonErrorCode::InputLimit, "E_CSON_INPUT_LIMIT"),
        (CsonErrorCode::StringLimit, "E_CSON_STRING_LIMIT"),
        (CsonErrorCode::ValueLimit, "E_CSON_VALUE_LIMIT"),
        (CsonErrorCode::ContainerLimit, "E_CSON_CONTAINER_LIMIT"),
        (CsonErrorCode::DepthLimit, "E_CSON_DEPTH_LIMIT"),
        (CsonErrorCode::WorkLimit, "E_CSON_WORK_LIMIT"),
        (CsonErrorCode::DiagnosticLimit, "E_CSON_DIAGNOSTIC_LIMIT"),
    ];
    for (code, expected) in cases {
        assert_eq!(code.as_str(), expected);
        assert_eq!(code.to_string(), expected);
    }
}

#[test]
fn errors_report_original_crlf_bytes_and_unicode_scalar_columns() {
    let source = "'é': 1\r\n'é': 2";
    let error = parse_cson(source).expect_err("the decoded key is duplicated");
    assert_eq!(error.code(), CsonErrorCode::DuplicateKey);
    assert_eq!(error.byte_range(), 9..13);
    assert_eq!(error.span().start.line, 2);
    assert_eq!(error.span().start.column, Some(1));
    let end = error.span().end.expect("CSON errors have a range");
    assert_eq!(end.line, 2);
    assert_eq!(end.column, Some(4));
    assert!(error.message().contains("duplicate"));
    assert!(
        error
            .to_string()
            .starts_with("E_CSON_DUPLICATE_KEY at 2:1:")
    );
    let _: &dyn std::error::Error = &error;
}

#[test]
fn input_string_value_container_and_depth_boundaries_are_exact() {
    let defaults = CsonLimits::default();

    assert_eq!(
        error_code("0", defaults.with_max_input_bytes(0)),
        CsonErrorCode::InputLimit
    );
    assert!(parse_cson_with_limits("0", defaults.with_max_input_bytes(1)).is_ok());
    assert!(parse_cson_with_limits("0", defaults.with_max_input_bytes(2)).is_ok());

    assert_eq!(
        error_code("'é'", defaults.with_max_string_bytes(1)),
        CsonErrorCode::StringLimit
    );
    assert!(parse_cson_with_limits("'é'", defaults.with_max_string_bytes(2)).is_ok());
    assert!(parse_cson_with_limits("'é'", defaults.with_max_string_bytes(3)).is_ok());
    assert_eq!(
        error_code("abc: 0", defaults.with_max_string_bytes(2)),
        CsonErrorCode::StringLimit
    );
    assert!(parse_cson_with_limits("abc: 0", defaults.with_max_string_bytes(3)).is_ok());

    assert_eq!(
        error_code("null", defaults.with_max_values(0)),
        CsonErrorCode::ValueLimit
    );
    assert!(parse_cson_with_limits("null", defaults.with_max_values(1)).is_ok());
    assert!(parse_cson_with_limits("null", defaults.with_max_values(2)).is_ok());
    assert_eq!(
        error_code("[0]", defaults.with_max_values(1)),
        CsonErrorCode::ValueLimit
    );
    assert!(parse_cson_with_limits("[0]", defaults.with_max_values(2)).is_ok());

    assert_eq!(
        error_code("[0]", defaults.with_max_container_items(0)),
        CsonErrorCode::ContainerLimit
    );
    assert!(parse_cson_with_limits("[0]", defaults.with_max_container_items(1)).is_ok());
    assert!(parse_cson_with_limits("[0]", defaults.with_max_container_items(2)).is_ok());
    assert_eq!(
        error_code("a: 0", defaults.with_max_container_items(0)),
        CsonErrorCode::ContainerLimit
    );
    assert!(parse_cson_with_limits("a: 0", defaults.with_max_container_items(1)).is_ok());

    assert_eq!(
        error_code("[[]]", defaults.with_max_nesting_depth(1)),
        CsonErrorCode::DepthLimit
    );
    assert!(parse_cson_with_limits("[[]]", defaults.with_max_nesting_depth(2)).is_ok());
    assert!(parse_cson_with_limits("[[]]", defaults.with_max_nesting_depth(3)).is_ok());
}

#[test]
fn integer_decimal_and_work_boundaries_are_exact() {
    let defaults = CsonLimits::default();

    assert_eq!(
        error_code("4", defaults.with_max_integer_bits(2)),
        CsonErrorCode::Number
    );
    assert!(parse_cson_with_limits("4", defaults.with_max_integer_bits(3)).is_ok());
    assert!(parse_cson_with_limits("4", defaults.with_max_integer_bits(4)).is_ok());

    assert_eq!(
        error_code("4.1", defaults.with_max_decimal_coefficient_bits(5)),
        CsonErrorCode::Number
    );
    assert!(parse_cson_with_limits("4.1", defaults.with_max_decimal_coefficient_bits(6)).is_ok());
    assert!(parse_cson_with_limits("4.1", defaults.with_max_decimal_coefficient_bits(7)).is_ok());

    assert_eq!(
        error_code("0.001", defaults.with_max_decimal_scale(2)),
        CsonErrorCode::Number
    );
    assert!(parse_cson_with_limits("0.001", defaults.with_max_decimal_scale(3)).is_ok());
    assert!(parse_cson_with_limits("0.001", defaults.with_max_decimal_scale(4)).is_ok());

    assert_eq!(
        error_code("null", defaults.with_max_work_units(4)),
        CsonErrorCode::WorkLimit
    );
    assert!(parse_cson_with_limits("null", defaults.with_max_work_units(5)).is_ok());
    assert!(parse_cson_with_limits("null", defaults.with_max_work_units(6)).is_ok());
}

#[test]
fn diagnostic_and_reserved_output_boundaries_are_independent() {
    let defaults = CsonLimits::default();
    assert_eq!(
        error_code("", defaults.with_max_diagnostics(0)),
        CsonErrorCode::DiagnosticLimit
    );
    assert_eq!(
        error_code("", defaults.with_max_diagnostics(1)),
        CsonErrorCode::Syntax
    );
    assert_eq!(
        error_code("", defaults.with_max_diagnostics(2)),
        CsonErrorCode::Syntax
    );

    // Output is deliberately reserved for the follow-up serializer and cannot
    // constrain this parser-only API.
    for boundary in [0, 1, 2] {
        assert!(parse_cson_with_limits("0", defaults.with_max_output_bytes(boundary)).is_ok());
    }
}

#[test]
fn exact_numbers_never_become_binary_numbers() {
    let values = parse_cson("[4, 4.1, -0, -0.0]")
        .expect("exact CSON numbers parse")
        .as_array()
        .expect("root is an Array")
        .to_vec();
    assert_eq!(values[0].kind(), ValueKind::Integer);
    assert_eq!(values[0].as_integer().unwrap().as_i64(), Some(4));
    assert_eq!(values[1].kind(), ValueKind::Decimal);
    assert_eq!(values[1].as_decimal().unwrap().to_plain_string(), "4.1");
    assert_eq!(values[2].kind(), ValueKind::Integer);
    assert_eq!(values[2].as_integer().unwrap().as_i64(), Some(0));
    assert_eq!(values[3].kind(), ValueKind::Decimal);
    assert_eq!(values[3].as_decimal().unwrap().to_plain_string(), "0");
    assert!(values.iter().all(|value| value.kind() != ValueKind::Number));
}

#[test]
fn malformed_text_and_deep_or_long_inputs_never_panic() {
    assert!(parse_cson("root:\n  message: '''\n    hello\n    '''").is_ok());
    assert!(parse_cson("'''\nroot text\n''' ").is_ok());

    let malformed = [
        "\u{feff}null",
        "'bad\\q'",
        "'bad\\uD800'",
        "'bad\\uDC00'",
        "'bad\\uD800\\u0041'",
        "'physical\nnewline'",
        "root:\n  child: 1\n   fractional: 2",
        "first:\n  child: 1\nsecond:\n    skipped: 1",
        "seed:\n  child: 0\narray: [\n    0\n]",
        "seed:\n  child: 0\narray: [\n  0\n  ]",
        "seed:\n  child: 0\nmessage: '''\n    skipped\n    '''",
        "null\rnext",
    ];
    for source in malformed {
        let result = catch_unwind(AssertUnwindSafe(|| parse_cson(source)));
        assert!(result.is_ok(), "parser panicked for {source:?}");
        assert!(result.unwrap().is_err(), "parser accepted {source:?}");
    }

    let allowed = format!("{}null{}", "[".repeat(128), "]".repeat(128));
    let result = catch_unwind(AssertUnwindSafe(|| parse_cson(&allowed)));
    assert!(result.is_ok(), "maximum-depth CSON input panicked");
    assert!(
        result.unwrap().is_ok(),
        "maximum depth must remain accepted"
    );

    let deep = format!("{}null{}", "[".repeat(600), "]".repeat(600));
    let result = catch_unwind(AssertUnwindSafe(|| {
        parse_cson_with_limits(
            &deep,
            CsonLimits::default().with_max_nesting_depth(usize::MAX),
        )
    }));
    assert!(result.is_ok(), "deep CSON input panicked");
    assert_eq!(
        result.unwrap().unwrap_err().code(),
        CsonErrorCode::DepthLimit
    );

    let long = format!("'{}'", "é".repeat(10_000));
    let result = catch_unwind(AssertUnwindSafe(|| {
        parse_cson_with_limits(&long, CsonLimits::default().with_max_string_bytes(19_999))
    }));
    assert!(result.is_ok(), "long CSON token panicked");
    assert_eq!(
        result.unwrap().unwrap_err().code(),
        CsonErrorCode::StringLimit
    );
}

#[test]
fn deterministic_stress_input_has_no_partial_or_unstable_result() {
    let source = format!(
        "[{}]",
        (0..2_000).map(|_| "0").collect::<Vec<_>>().join(",")
    );
    for _ in 0..3 {
        let result = catch_unwind(AssertUnwindSafe(|| parse_cson(&source)));
        let value = result
            .expect("stress parse must not panic")
            .expect("stress parse must succeed");
        let values = value.as_array().expect("stress root is an Array");
        assert_eq!(values.len(), 2_000);
        assert!(values.iter().all(|value| {
            value
                .as_integer()
                .is_some_and(|integer| integer.as_i64() == Some(0))
        }));
    }
}
