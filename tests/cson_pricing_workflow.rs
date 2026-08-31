use quickcoffee::{
    CsonErrorCode, CsonLimits, Decimal, ExecutionPolicy, RestrictedFileModuleLoader, Runtime,
    Value, parse_cson, parse_cson_with_limits,
};
use std::{collections::BTreeMap, fs, path::PathBuf, process::Command};

const CANONICAL_CONFIG_JSON: &str = concat!(
    "{\"accepted\":{\"country\":\"CN\",\"customer_tier\":\"member\",",
    "\"item_count\":3,\"subtotal\":\"120\"},\"rejected\":{\"country\":\"US\",",
    "\"customer_tier\":\"standard\",\"item_count\":1,\"subtotal\":\"5\"},",
    "\"schema\":\"pricing-orders/v1\"}\n"
);
const EXPECTED_RESULT: &str = concat!(
    "{quote: {discount: 12m, net: 108m, subtotal: 120m, tax: 14.04m, ",
    "total: 122.04m}, rejection: pricing.ineligible}\n"
);

fn repository(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path)
}

fn run_qcson(arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_qcson"))
        .args(arguments)
        .output()
        .expect("qcson starts")
}

fn run_configured(json: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_qcoffee"))
        .args(["--module-root"])
        .arg(repository("examples/pricing"))
        .args(["configured", "--", json.trim_end()])
        .output()
        .expect("qcoffee starts")
}

fn required_string<'a>(map: &'a BTreeMap<String, Value>, field: &str) -> &'a str {
    map.get(field).and_then(Value::as_str).unwrap()
}

fn configured_order(config: &Value, section: &str) -> Value {
    let root = config.as_map().unwrap();
    assert_eq!(root["schema"].as_str(), Some("pricing-orders/v1"));
    let order = root[section].as_map().unwrap();
    Value::map([
        (
            "subtotal",
            Value::from(Decimal::parse(required_string(order, "subtotal")).unwrap()),
        ),
        (
            "item_count",
            Value::from(order["item_count"].as_integer().unwrap().as_i64().unwrap()),
        ),
        (
            "customer_tier",
            Value::from(required_string(order, "customer_tier")),
        ),
        ("country", Value::from(required_string(order, "country"))),
    ])
}

fn embedded_result(config: &Value) -> Value {
    let runtime = Runtime::builder()
        .execution_policy(ExecutionPolicy::isolated_request())
        .build();
    let loader = RestrictedFileModuleLoader::new(repository("examples/pricing")).unwrap();
    let source = loader.load_entry("host").unwrap();
    let entry = runtime
        .compile_module(source.name(), source.source())
        .unwrap();
    let package = runtime.prepare_module_package(&entry, &loader).unwrap();
    let run = |request| {
        let mut context = runtime.context_builder().global("request", request).build();
        context
            .run_module_package(&package)
            .unwrap()
            .get("result")
            .cloned()
            .unwrap()
    };
    let quote = run(configured_order(config, "accepted"));
    let mut context = runtime
        .context_builder()
        .global("request", configured_order(config, "rejected"))
        .build();
    let error = context.run_module_package(&package).unwrap_err();
    let rejection = error.script_error().unwrap().code().to_owned();
    Value::map([("quote", quote), ("rejection", Value::from(rejection))])
}

#[test]
fn cson_to_pricing_cli_is_byte_stable_across_three_runs() {
    let config = repository("examples/pricing/config.cson");
    for _ in 0..3 {
        let converted = run_qcson(&["to-json", config.to_str().unwrap()]);
        assert!(
            converted.status.success(),
            "{}",
            String::from_utf8_lossy(&converted.stderr)
        );
        assert!(converted.stderr.is_empty());
        assert_eq!(
            String::from_utf8_lossy(&converted.stdout),
            CANONICAL_CONFIG_JSON
        );

        let priced = run_configured(CANONICAL_CONFIG_JSON);
        assert!(
            priced.status.success(),
            "{}",
            String::from_utf8_lossy(&priced.stderr)
        );
        assert!(priced.stderr.is_empty());
        assert_eq!(String::from_utf8_lossy(&priced.stdout), EXPECTED_RESULT);
    }
}

#[test]
fn cson_values_actually_determine_the_quote() {
    let original = fs::read_to_string(repository("examples/pricing/config.cson")).unwrap();
    let changed = original.replacen("subtotal: '120'", "subtotal: '200'", 1);
    let path = std::env::temp_dir().join(format!(
        "quickcoffee-pricing-config-{}.cson",
        std::process::id()
    ));
    fs::write(&path, changed).unwrap();
    let converted = run_qcson(&["to-json", path.to_str().unwrap()]);
    let _ = fs::remove_file(path);
    assert!(converted.status.success());
    let json = String::from_utf8(converted.stdout).unwrap();
    let priced = run_configured(&json);
    assert!(priced.status.success());
    assert_eq!(
        String::from_utf8(priced.stdout).unwrap(),
        concat!(
            "{quote: {discount: 20m, net: 180m, subtotal: 200m, tax: 23.4m, ",
            "total: 203.4m}, rejection: pricing.ineligible}\n"
        )
    );
}

#[test]
fn rust_embedding_and_cli_agree_on_quote_and_rejection() {
    let source = fs::read_to_string(repository("examples/pricing/config.cson")).unwrap();
    let config = parse_cson(&source).unwrap();
    let rust = format!("{}\n", embedded_result(&config));
    let cli = run_configured(CANONICAL_CONFIG_JSON);
    assert!(cli.status.success());
    assert_eq!(rust.as_bytes(), cli.stdout);
    assert_eq!(rust, EXPECTED_RESULT);
}

#[test]
fn workflow_reports_syntax_and_resource_failures_without_partial_output() {
    let syntax = run_qcson(&[
        "to-json",
        repository("tests/cson/reject/arithmetic.cson")
            .to_str()
            .unwrap(),
    ]);
    assert_eq!(syntax.status.code(), Some(1));
    assert!(syntax.stdout.is_empty());
    assert!(String::from_utf8_lossy(&syntax.stderr).contains("E_CSON_EXPRESSION"));

    let source = fs::read_to_string(repository("examples/pricing/config.cson")).unwrap();
    let error = parse_cson_with_limits(
        &source,
        CsonLimits::default().with_max_input_bytes(source.len() - 1),
    )
    .unwrap_err();
    assert_eq!(error.code(), CsonErrorCode::InputLimit);

    let resource = run_qcson(&[
        "--max-input-bytes",
        &(source.len() - 1).to_string(),
        "to-json",
        repository("examples/pricing/config.cson").to_str().unwrap(),
    ]);
    assert_eq!(resource.status.code(), Some(1));
    assert!(resource.stdout.is_empty());
    assert!(String::from_utf8_lossy(&resource.stderr).contains("E_CSON_INPUT_LIMIT"));
}

#[test]
fn configured_module_only_consumes_explicit_argv_data() {
    let source = fs::read_to_string(repository("examples/pricing/configured.coffee")).unwrap();
    assert!(source.contains("config = parse_json(argv[0])"));
    assert_eq!(source.matches("import ").count(), 1);
    assert!(source.contains("from './rule.litcoffee'"));
}
