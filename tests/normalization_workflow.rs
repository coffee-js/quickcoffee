use quickcoffee::{
    Error, ErrorKind, ExecutionPolicy, ModulePackage, ResourceLimit, ResourceLimits,
    RestrictedFileModuleLoader, Runtime, Value,
};
use std::{fs, path::PathBuf, process::Command};

const SAMPLE_INPUT: &str = include_str!("../examples/normalization/input.v1.json");
const SAMPLE_OUTPUT: &str = include_str!("../examples/normalization/expected.v1.json");

fn prepare(
    runtime: &Runtime,
    loader: &RestrictedFileModuleLoader,
    entry_name: &str,
) -> ModulePackage {
    let source = loader.load_entry(entry_name).unwrap();
    let entry = runtime
        .compile_module(source.name(), source.source())
        .unwrap();
    runtime.prepare_module_package(&entry, loader).unwrap()
}

fn package(runtime: &Runtime) -> (ModulePackage, RestrictedFileModuleLoader) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/normalization");
    let loader = RestrictedFileModuleLoader::new(root).unwrap();
    let package = prepare(runtime, &loader, "host");
    (package, loader)
}

fn isolated_runtime() -> Runtime {
    Runtime::builder()
        .execution_policy(ExecutionPolicy::isolated_request())
        .build()
}

fn run(
    runtime: &Runtime,
    package: &ModulePackage,
    source: &str,
    limits: Option<ResourceLimits>,
) -> Result<String, Error> {
    let mut builder = runtime
        .context_builder()
        .global("input_json", Value::from(source));
    if let Some(limits) = limits {
        builder = builder.resource_limits(limits);
    }
    let mut context = builder.build();
    context
        .run_module_package(package)?
        .get("result")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| Error::runtime("missing normalization result"))
}

#[test]
fn normalization_is_exact_deterministic_and_context_isolated() {
    let runtime = Runtime::builder()
        .execution_policy(ExecutionPolicy::isolated_request())
        .module_cache_entries(8)
        .build();
    let (package, loader) = package(&runtime);
    let expected = SAMPLE_OUTPUT.trim_end();

    let corpus_package = prepare(&runtime, &loader, "corpus");
    let mut corpus_context = runtime.new_context();
    let corpus = corpus_context.run_module_package(&corpus_package).unwrap();
    assert_eq!(
        corpus.get("sample_input").unwrap().as_str().unwrap(),
        SAMPLE_INPUT.trim_end()
    );
    assert_eq!(
        corpus.get("sample_output").unwrap().as_str().unwrap(),
        expected
    );

    let first = run(&runtime, &package, SAMPLE_INPUT, None).unwrap();
    assert_eq!(first, expected);
    let second = run(&runtime, &package, SAMPLE_INPUT, None).unwrap();
    assert_eq!(second, first);
}

#[test]
fn normalization_reports_json_domain_and_resource_failures() {
    let runtime = isolated_runtime();
    let (package, _) = package(&runtime);

    let rule_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/normalization/rule.litcoffee");
    let rule = fs::read_to_string(rule_path).unwrap();
    let broken_rule = rule.replacen("sort(normalized)", "sort(normalized", 1);
    let syntax = runtime
        .compile_module("broken-rule.litcoffee", &broken_rule)
        .unwrap_err();
    assert_eq!(syntax.kind(), ErrorKind::Parse);
    assert_eq!(
        syntax.labels()[0].span.source_name.as_deref(),
        Some("broken-rule.litcoffee")
    );
    assert!(syntax.labels()[0].span.start.line > 1);

    let malformed = run(&runtime, &package, "{\"schema\":", None).unwrap_err();
    assert_eq!(malformed.script_error().unwrap().code(), "json.parse");

    let unsupported = run(
        &runtime,
        &package,
        r#"{"events":[],"schema":"profile-events/v2"}"#,
        None,
    )
    .unwrap_err();
    assert_eq!(
        unsupported.script_error().unwrap().code(),
        "normalization.unsupported_schema"
    );
    assert_eq!(
        unsupported.labels()[0].span.source_name.as_deref(),
        Some("rule.litcoffee")
    );

    let invalid = run(
        &runtime,
        &package,
        r#"{"events":[{"amount":1.0,"id":"event","metadata":{"source":"host"},"name":"name","sequence":1,"tags":[1]}],"schema":"profile-events/v1"}"#,
        None,
    )
    .unwrap_err();
    let script = invalid.script_error().unwrap();
    assert_eq!(script.code(), "normalization.invalid_event");
    assert_eq!(
        script.data().as_map().unwrap()["field"].as_str(),
        Some("tags[0]")
    );

    let missing = run(
        &runtime,
        &package,
        r#"{"events":[{}],"schema":"profile-events/v1"}"#,
        None,
    )
    .unwrap_err();
    let script = missing.script_error().unwrap();
    assert_eq!(script.code(), "normalization.invalid_event");
    assert_eq!(
        script.data().as_map().unwrap()["field"].as_str(),
        Some("id")
    );
    assert_eq!(
        script.data().as_map().unwrap()["expected"].as_str(),
        Some("required")
    );

    let limits = ResourceLimits::default().with_max_json_input_bytes(32);
    let resource = run(&runtime, &package, SAMPLE_INPUT, Some(limits)).unwrap_err();
    assert_eq!(resource.kind(), ErrorKind::Resource);
    assert_eq!(
        resource.resource_limit(),
        Some(ResourceLimit::JsonInputBytes)
    );

    let limits = ResourceLimits::default().with_max_json_nesting_depth(1);
    let resource = run(&runtime, &package, SAMPLE_INPUT, Some(limits)).unwrap_err();
    assert_eq!(resource.kind(), ErrorKind::Resource);
    assert_eq!(
        resource.resource_limit(),
        Some(ResourceLimit::JsonNestingDepth)
    );
}

#[test]
fn normalization_cli_and_qtest_paths_are_reproducible() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/normalization");
    let root_text = root.to_str().unwrap();

    let checked = Command::new(env!("CARGO_BIN_EXE_qcoffee"))
        .args(["--fingerprint", "--module-root", root_text, "demo"])
        .output()
        .unwrap();
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    let fingerprint = String::from_utf8(checked.stdout).unwrap();
    assert_eq!(fingerprint.trim().len(), 16);
    assert!(
        fingerprint
            .trim()
            .chars()
            .all(|digit| digit.is_ascii_hexdigit())
    );

    let tested = Command::new(env!("CARGO_BIN_EXE_qtest"))
        .args(["--module-root", root_text, "test"])
        .output()
        .unwrap();
    assert!(
        tested.status.success(),
        "{}",
        String::from_utf8_lossy(&tested.stderr)
    );
    assert_eq!(
        String::from_utf8(tested.stdout).unwrap(),
        "ok test.coffee\n"
    );

    let executed = Command::new(env!("CARGO_BIN_EXE_qcoffee"))
        .args(["--json", "--module-root", root_text, "demo"])
        .output()
        .unwrap();
    assert!(
        executed.status.success(),
        "{}",
        String::from_utf8_lossy(&executed.stderr)
    );
    let stdout = String::from_utf8(executed.stdout).unwrap();
    let escaped = SAMPLE_OUTPUT
        .trim_end()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    assert_eq!(
        stdout,
        format!("{{\"ok\":true,\"exports\":{{\"normalized\":\"{escaped}\"}}}}\n")
    );
}
