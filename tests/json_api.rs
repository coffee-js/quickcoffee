use quickcoffee::{
    JsonErrorCode, ResourceLimit, ResourceLimits, Value, encode_json, encode_json_with_limits,
    parse_cson, parse_json, parse_json_with_limits,
};

#[test]
fn public_json_api_preserves_exact_values_and_canonical_bytes() {
    let source = r#"{"z":2,"large":9007199254740993,"money":12.30,"nested":[true,null]}"#;
    let value = parse_json(source).expect("public default parser accepts exact JSON");
    assert_eq!(
        encode_json(&value).expect("public default encoder emits canonical JSON"),
        r#"{"large":9007199254740993,"money":12.3,"nested":[true,null],"z":2}"#
    );

    let cson = "large: 9007199254740993\nmoney: 12.3\nnested: [\n  true\n  null\n]\nz: 2\n";
    assert_eq!(
        encode_json(&parse_cson(cson).unwrap()).unwrap(),
        encode_json(&value).unwrap()
    );
}

#[test]
fn public_json_error_categories_offsets_and_resources_are_structured() {
    let syntax = parse_json("[1,]").unwrap_err();
    assert_eq!(syntax.code(), JsonErrorCode::Syntax);
    assert_eq!(syntax.code().as_str(), "E_JSON_SYNTAX");
    assert_eq!(syntax.byte_offset(), Some(3));
    assert_eq!(syntax.resource_limit(), None);

    let resource = parse_json_with_limits(
        "null",
        ResourceLimits::default().with_max_json_input_bytes(3),
    )
    .unwrap_err();
    assert_eq!(resource.code(), JsonErrorCode::Resource);
    assert_eq!(resource.code().as_str(), "E_JSON_RESOURCE");
    assert_eq!(
        resource.resource_limit(),
        Some(ResourceLimit::JsonInputBytes)
    );
    assert_eq!(resource.byte_offset(), None);

    let value_type = encode_json(&Value::from(f64::NAN)).unwrap_err();
    assert_eq!(value_type.code(), JsonErrorCode::Type);
    assert_eq!(value_type.code().as_str(), "E_JSON_TYPE");
    assert_eq!(value_type.resource_limit(), None);
    assert_eq!(value_type.byte_offset(), None);
}

#[test]
fn public_json_explicit_limits_are_exact_and_atomic() {
    let input = ResourceLimits::default().with_max_json_input_bytes(4);
    assert!(parse_json_with_limits("null", input).is_ok());
    assert_eq!(
        parse_json_with_limits("null", input.with_max_json_input_bytes(3))
            .unwrap_err()
            .resource_limit(),
        Some(ResourceLimit::JsonInputBytes)
    );

    let output = ResourceLimits::default().with_max_json_output_bytes(4);
    assert_eq!(
        encode_json_with_limits(&Value::Bool(true), output).unwrap(),
        "true"
    );
    assert_eq!(
        encode_json_with_limits(&Value::Bool(true), output.with_max_json_output_bytes(3))
            .unwrap_err()
            .resource_limit(),
        Some(ResourceLimit::JsonOutputBytes)
    );
}
