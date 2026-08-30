use std::{
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};

use quickcoffee::{
    Context, CsonErrorCode, CsonLimits, Decimal, Value, ValueKind, parse_cson, to_cson,
    to_cson_with_limits,
};

fn serialization_error(value: &Value, limits: CsonLimits) -> CsonErrorCode {
    to_cson_with_limits(value, limits)
        .expect_err("value must exceed the tested serializer boundary")
        .code()
}

fn nested_arrays(depth: usize) -> Value {
    let mut value = Value::Nil;
    for _ in 0..depth {
        value = Value::array([value]);
    }
    value
}

#[test]
fn canonical_golden_covers_maps_arrays_keys_strings_and_exact_numbers() {
    let value = Value::map([
        ("z", Value::integer(2_i64)),
        ("a", Value::integer(1_i64)),
        ("dash-key", Value::from("quoted")),
        ("control", Value::from("\u{0000}\t'é\\")),
        ("empty", Value::map(Vec::<(String, Value)>::new())),
        ("multiline", Value::from("first\nsecond")),
        ("fallback", Value::from("first\n'''second")),
        (
            "array",
            Value::array([
                Value::from(true),
                Value::from(Decimal::parse("1.0").unwrap()),
                Value::map(Vec::<(String, Value)>::new()),
            ]),
        ),
        ("nested", Value::map([("inner", Value::integer(9_i64))])),
        ("é", Value::from("accent")),
        ("空", Value::from("space")),
    ]);

    let expected = concat!(
        "a: 1\n",
        "array: [\n",
        "  true\n",
        "  1.0\n",
        "  {}\n",
        "]\n",
        "control: '\\u0000\\t\\'é\\\\'\n",
        "'dash-key': 'quoted'\n",
        "empty: {}\n",
        "fallback: 'first\\n\\'\\'\\'second'\n",
        "multiline: '''\n",
        "  first\n",
        "  second\n",
        "  '''\n",
        "nested:\n",
        "  inner: 9\n",
        "z: 2\n",
        "'é': 'accent'\n",
        "'空': 'space'\n",
    );
    let encoded = to_cson(&value).expect("supported data serializes");
    assert_eq!(encoded, expected);
    assert_eq!(to_cson(&parse_cson(&encoded).unwrap()).unwrap(), expected);
}

#[test]
fn root_scalars_empty_containers_and_string_fallbacks_are_canonical() {
    assert_eq!(to_cson(&Value::Nil).unwrap(), "null\n");
    assert_eq!(
        to_cson(&Value::map(Vec::<(String, Value)>::new())).unwrap(),
        "{}\n"
    );
    assert_eq!(to_cson(&Value::array([])).unwrap(), "[]\n");
    assert_eq!(to_cson(&Value::from("coffee")).unwrap(), "'coffee'\n");
    assert_eq!(to_cson(&Value::from("a\rb")).unwrap(), "'a\\rb'\n");
    assert_eq!(
        to_cson(&Value::from("root\ntext")).unwrap(),
        "'''\nroot\ntext\n'''\n"
    );
    assert_eq!(
        to_cson(&Value::from("root\n'''text")).unwrap(),
        "'root\\n\\'\\'\\'text'\n"
    );
}

#[test]
fn consecutive_array_maps_use_braces_to_preserve_item_boundaries() {
    let value = Value::array([
        Value::map([("a", Value::integer(1_i64))]),
        Value::map([("b", Value::integer(2_i64))]),
    ]);
    let expected = concat!(
        "[\n",
        "  {\n",
        "    a: 1\n",
        "  }\n",
        "  {\n",
        "    b: 2\n",
        "  }\n",
        "]\n",
    );
    let encoded = to_cson(&value).unwrap();
    assert_eq!(encoded, expected);
    let reparsed = parse_cson(&encoded).unwrap();
    let items = reparsed.as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(
        items[0].as_map().unwrap()["a"]
            .as_integer()
            .unwrap()
            .as_i64(),
        Some(1)
    );
    assert_eq!(
        items[1].as_map().unwrap()["b"]
            .as_integer()
            .unwrap()
            .as_i64(),
        Some(2)
    );
    assert_eq!(to_cson(&reparsed).unwrap(), expected);
}

#[test]
fn every_unsupported_value_kind_returns_the_stable_type_code() {
    let cases = [
        Value::from(1.5_f64),
        Context::new()
            .eval("error('cson.test', 'unsupported')")
            .unwrap(),
        Context::new().eval("-> 1").unwrap(),
        Context::new().eval("class Empty\nEmpty").unwrap(),
        Context::new().eval("class Empty\nnew Empty()").unwrap(),
    ];
    assert_eq!(
        cases.iter().map(Value::kind).collect::<Vec<_>>(),
        [
            ValueKind::Number,
            ValueKind::Error,
            ValueKind::Function,
            ValueKind::Class,
            ValueKind::Instance,
        ]
    );
    for value in cases {
        let error = to_cson(&value).expect_err("non-data Value must be rejected");
        assert_eq!(error.code(), CsonErrorCode::Type);
        assert_eq!(error.byte_range(), 0..0);
        assert_eq!(error.span().start.line, 1);
        assert_eq!(error.span().start.column, Some(1));
    }
}

#[test]
fn all_active_serializer_resource_boundaries_are_exact() {
    let defaults = CsonLimits::default();
    let nil = Value::Nil;

    assert_eq!(
        serialization_error(&nil, defaults.with_max_output_bytes(4)),
        CsonErrorCode::OutputLimit
    );
    assert_eq!(
        to_cson_with_limits(&nil, defaults.with_max_output_bytes(5)).unwrap(),
        "null\n"
    );
    assert!(to_cson_with_limits(&nil, defaults.with_max_output_bytes(6)).is_ok());

    let text = Value::from("é");
    assert_eq!(
        serialization_error(&text, defaults.with_max_string_bytes(1)),
        CsonErrorCode::StringLimit
    );
    assert!(to_cson_with_limits(&text, defaults.with_max_string_bytes(2)).is_ok());
    assert!(to_cson_with_limits(&text, defaults.with_max_string_bytes(3)).is_ok());

    assert_eq!(
        serialization_error(&nil, defaults.with_max_values(0)),
        CsonErrorCode::ValueLimit
    );
    assert!(to_cson_with_limits(&nil, defaults.with_max_values(1)).is_ok());
    assert!(to_cson_with_limits(&nil, defaults.with_max_values(2)).is_ok());

    let one = Value::array([Value::Nil]);
    assert_eq!(
        serialization_error(&one, defaults.with_max_container_items(0)),
        CsonErrorCode::ContainerLimit
    );
    assert!(to_cson_with_limits(&one, defaults.with_max_container_items(1)).is_ok());
    assert!(to_cson_with_limits(&one, defaults.with_max_container_items(2)).is_ok());

    let nested = nested_arrays(2);
    assert_eq!(
        serialization_error(&nested, defaults.with_max_nesting_depth(1)),
        CsonErrorCode::DepthLimit
    );
    assert!(to_cson_with_limits(&nested, defaults.with_max_nesting_depth(2)).is_ok());
    assert!(to_cson_with_limits(&nested, defaults.with_max_nesting_depth(3)).is_ok());

    let integer = Value::integer(4_i64);
    assert_eq!(
        serialization_error(&integer, defaults.with_max_integer_bits(2)),
        CsonErrorCode::Number
    );
    assert!(to_cson_with_limits(&integer, defaults.with_max_integer_bits(3)).is_ok());
    assert!(to_cson_with_limits(&integer, defaults.with_max_integer_bits(4)).is_ok());

    let decimal = Value::from(Decimal::parse("4.1").unwrap());
    assert_eq!(
        serialization_error(&decimal, defaults.with_max_decimal_coefficient_bits(5)),
        CsonErrorCode::Number
    );
    assert!(to_cson_with_limits(&decimal, defaults.with_max_decimal_coefficient_bits(6)).is_ok());
    assert!(to_cson_with_limits(&decimal, defaults.with_max_decimal_coefficient_bits(7)).is_ok());

    let scaled = Value::from(Decimal::parse("0.001").unwrap());
    assert_eq!(
        serialization_error(&scaled, defaults.with_max_decimal_scale(2)),
        CsonErrorCode::Number
    );
    assert!(to_cson_with_limits(&scaled, defaults.with_max_decimal_scale(3)).is_ok());
    assert!(to_cson_with_limits(&scaled, defaults.with_max_decimal_scale(4)).is_ok());

    assert_eq!(
        serialization_error(&nil, defaults.with_max_work_units(5)),
        CsonErrorCode::WorkLimit
    );
    assert!(to_cson_with_limits(&nil, defaults.with_max_work_units(6)).is_ok());
    assert!(to_cson_with_limits(&nil, defaults.with_max_work_units(7)).is_ok());

    let number = Value::from(1.5_f64);
    assert_eq!(
        serialization_error(&number, defaults.with_max_diagnostics(0)),
        CsonErrorCode::DiagnosticLimit
    );
    assert_eq!(
        serialization_error(&number, defaults.with_max_diagnostics(1)),
        CsonErrorCode::Type
    );
    assert_eq!(
        serialization_error(&number, defaults.with_max_diagnostics(2)),
        CsonErrorCode::Type
    );

    for boundary in [0, 1, 2] {
        assert!(to_cson_with_limits(&nil, defaults.with_max_input_bytes(boundary)).is_ok());
    }
}

#[test]
fn map_key_limits_are_checked_before_output() {
    let value = Value::Map(std::rc::Rc::new(BTreeMap::from([(
        "é".to_owned(),
        Value::Nil,
    )])));
    assert_eq!(
        serialization_error(&value, CsonLimits::default().with_max_string_bytes(1)),
        CsonErrorCode::StringLimit
    );
    assert!(to_cson_with_limits(&value, CsonLimits::default().with_max_string_bytes(2)).is_ok());
}

#[test]
fn depth_and_deterministic_stress_metrics_match_without_panics() {
    let allowed = nested_arrays(128);
    let result = catch_unwind(AssertUnwindSafe(|| to_cson(&allowed)));
    let encoded = result
        .expect("depth-128 serialization must not panic")
        .expect("depth 128 must be accepted");
    assert!(parse_cson(&encoded).is_ok());

    let denied = nested_arrays(129);
    let result = catch_unwind(AssertUnwindSafe(|| {
        to_cson_with_limits(
            &denied,
            CsonLimits::default().with_max_nesting_depth(usize::MAX),
        )
    }));
    assert!(result.is_ok(), "depth-129 serialization must not panic");
    assert_eq!(
        result.unwrap().unwrap_err().code(),
        CsonErrorCode::DepthLimit
    );

    let stress = Value::array(
        (0..2_000)
            .map(|_| Value::integer(0_i64))
            .collect::<Vec<_>>(),
    );
    let first = to_cson(&stress).expect("2,000-item stress value serializes");
    for _ in 0..3 {
        let result = catch_unwind(AssertUnwindSafe(|| to_cson(&stress)));
        assert_eq!(
            result.expect("stress serializer must not panic").unwrap(),
            first
        );
    }
    assert_eq!(parse_cson(&first).unwrap().as_array().unwrap().len(), 2_000);
}
