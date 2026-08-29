use quickcoffee::{
    Decimal, Error, ErrorKind, ModulePackage, ResourceLimit, ResourceLimits,
    RestrictedFileModuleLoader, Runtime, Value,
};
use std::{path::PathBuf, process::Command};

fn decimal(source: &str) -> Value {
    Value::from(Decimal::parse(source).expect("valid test decimal"))
}

fn order(subtotal: Value, item_count: Value, customer_tier: Value, country: Value) -> Value {
    Value::map([
        ("subtotal", subtotal),
        ("item_count", item_count),
        ("customer_tier", customer_tier),
        ("country", country),
    ])
}

fn valid_order(subtotal: &str) -> Value {
    order(
        decimal(subtotal),
        Value::from(3_i64),
        Value::from("member"),
        Value::from("CN"),
    )
}

fn pricing_package(runtime: &Runtime) -> (ModulePackage, RestrictedFileModuleLoader) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/pricing");
    let loader = RestrictedFileModuleLoader::new(root).unwrap();
    let source = loader.load_entry("host").unwrap();
    let entry = runtime
        .compile_module(source.name(), source.source())
        .unwrap();
    let package = runtime.prepare_module_package(&entry, &loader).unwrap();
    (package, loader)
}

fn run(
    runtime: &Runtime,
    package: &ModulePackage,
    request: Value,
    limits: ResourceLimits,
) -> Result<Value, Error> {
    let mut context = runtime
        .context_builder()
        .fuel(100_000)
        .resource_limits(limits)
        .global("request", request)
        .build();
    context
        .run_module_package(package)?
        .get("result")
        .cloned()
        .ok_or_else(|| Error::runtime("missing pricing result"))
}

#[test]
fn literate_pricing_rule_produces_exact_deterministic_money() {
    let runtime = Runtime::new();
    let (package, _) = pricing_package(&runtime);
    let request = valid_order("120");
    let before = request.to_string();

    let result = run(
        &runtime,
        &package,
        request.clone(),
        ResourceLimits::default(),
    )
    .unwrap();
    let quote = result.as_map().unwrap();
    assert_eq!(
        quote["subtotal"].as_decimal().unwrap().to_plain_string(),
        "120"
    );
    assert_eq!(
        quote["discount"].as_decimal().unwrap().to_plain_string(),
        "12"
    );
    assert_eq!(quote["net"].as_decimal().unwrap().to_plain_string(), "108");
    assert_eq!(
        quote["tax"].as_decimal().unwrap().to_plain_string(),
        "14.04"
    );
    assert_eq!(
        quote["total"].as_decimal().unwrap().to_plain_string(),
        "122.04"
    );
    assert_eq!(
        request.to_string(),
        before,
        "the input value must remain immutable"
    );

    let second = run(&runtime, &package, request, ResourceLimits::default()).unwrap();
    assert_eq!(second.to_string(), result.to_string());
}

#[test]
fn pricing_package_is_reusable_across_isolated_requests() {
    let runtime = Runtime::builder().module_cache_entries(8).build();
    let (package, _) = pricing_package(&runtime);

    let member = run(
        &runtime,
        &package,
        valid_order("120"),
        ResourceLimits::default(),
    )
    .unwrap();
    let standard = order(
        decimal("120"),
        Value::from(3_i64),
        Value::from("standard"),
        Value::from("US"),
    );
    let standard = run(&runtime, &package, standard, ResourceLimits::default()).unwrap();

    assert_eq!(
        member.as_map().unwrap()["total"]
            .as_decimal()
            .unwrap()
            .to_plain_string(),
        "122.04"
    );
    assert_eq!(
        standard.as_map().unwrap()["total"]
            .as_decimal()
            .unwrap()
            .to_plain_string(),
        "128.4"
    );
}

#[test]
fn pricing_rule_separates_business_rejection_from_invalid_input() {
    let runtime = Runtime::new();
    let (package, _) = pricing_package(&runtime);
    let ineligible = order(
        decimal("5"),
        Value::from(1_i64),
        Value::from("standard"),
        Value::from("US"),
    );
    let error = run(&runtime, &package, ineligible, ResourceLimits::default()).unwrap_err();
    let script = error.script_error().unwrap();
    assert_eq!(script.code(), "pricing.ineligible");
    assert_eq!(
        script.data().as_map().unwrap()["minimum_subtotal"]
            .as_decimal()
            .unwrap()
            .to_plain_string(),
        "10"
    );

    let invalid = order(
        Value::from(120_f64),
        Value::from(3_i64),
        Value::from("member"),
        Value::from("CN"),
    );
    let error = run(&runtime, &package, invalid, ResourceLimits::default()).unwrap_err();
    let script = error.script_error().unwrap();
    assert_eq!(script.code(), "pricing.invalid_order");
    assert_eq!(
        script.data().as_map().unwrap()["field"].as_str(),
        Some("subtotal")
    );
}

#[test]
fn pricing_rule_rejects_missing_fields_and_honors_resource_policy() {
    let runtime = Runtime::new();
    let (package, _) = pricing_package(&runtime);
    let missing = Value::map([
        ("subtotal", decimal("120")),
        ("item_count", Value::from(3_i64)),
        ("customer_tier", Value::from("member")),
    ]);
    let error = run(&runtime, &package, missing, ResourceLimits::default()).unwrap_err();
    let script = error.script_error().unwrap();
    assert_eq!(script.code(), "pricing.invalid_order");
    assert_eq!(
        script.data().as_map().unwrap()["field"].as_str(),
        Some("country")
    );

    let limits = ResourceLimits::default().with_max_map_entries(3);
    let error = run(&runtime, &package, valid_order("120"), limits).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Resource);
    assert_eq!(error.resource_limit(), Some(ResourceLimit::MapEntries));
}

#[test]
fn pricing_cli_demo_is_one_command_and_machine_readable() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/pricing");
    let output = Command::new(env!("CARGO_BIN_EXE_qcoffee"))
        .args(["--json", "--module-root"])
        .arg(root)
        .arg("demo")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        concat!(
            "{\"ok\":true,\"exports\":{\"quote\":{",
            "\"discount\":{\"$quickcoffee\":\"decimal\",\"value\":\"12\"},",
            "\"net\":{\"$quickcoffee\":\"decimal\",\"value\":\"108\"},",
            "\"subtotal\":{\"$quickcoffee\":\"decimal\",\"value\":\"120\"},",
            "\"tax\":{\"$quickcoffee\":\"decimal\",\"value\":\"14.04\"},",
            "\"total\":{\"$quickcoffee\":\"decimal\",\"value\":\"122.04\"}},",
            "\"rejection\":\"pricing.ineligible\"}}\n"
        )
    );
}

#[test]
fn pricing_rule_runs_as_an_isolated_qtest_module_case() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/pricing");
    let output = Command::new(env!("CARGO_BIN_EXE_qtest"))
        .args(["--module-root"])
        .arg(root)
        .arg("test")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "ok test.coffee\n"
    );
}
