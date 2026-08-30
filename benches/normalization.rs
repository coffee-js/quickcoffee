//! Run with `cargo bench --bench normalization`.
//!
//! The benchmark parses, validates, reshapes, and canonically encodes the same
//! versioned JSON corpus used by the example and integration tests.
use quickcoffee::{ModulePackage, RestrictedFileModuleLoader, Runtime, Value};
use std::{hint::black_box, path::PathBuf, time::Instant};

const INPUT: &str = include_str!("../examples/normalization/input.v1.json");
const EXPECTED: &str = include_str!("../examples/normalization/expected.v1.json");
const SCALE_EVENT: &str = r#"{"amount":12.30,"id":" event ","metadata":{"channel":"bench","source":" host ","transient":true},"name":"　Coffee ☕　","note":" ready ","sequence":9007199254740993,"tags":["中","a","☕"]}"#;

fn loader() -> RestrictedFileModuleLoader {
    RestrictedFileModuleLoader::new(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/normalization"),
    )
    .expect("normalization example root is available")
}

fn prepare(runtime: &Runtime, loader: &RestrictedFileModuleLoader) -> ModulePackage {
    let source = loader.load_entry("host").expect("normalization host loads");
    let entry = runtime
        .compile_module(source.name(), source.source())
        .expect("normalization host compiles");
    runtime
        .prepare_module_package(&entry, loader)
        .expect("normalization package preflights")
}

fn scale_input(event_count: usize) -> String {
    format!(
        "{{\"events\":[{}],\"schema\":\"profile-events/v1\"}}",
        vec![SCALE_EVENT; event_count].join(",")
    )
}

fn execute(runtime: &Runtime, package: &ModulePackage, event_count: usize) {
    let requests = 10_000 / event_count;
    let total_events = requests * event_count;
    let input = Value::from(scale_input(event_count));
    let mut instructions = 0_u64;
    let mut managed_objects = 0_u64;
    let mut managed_bytes = 0_u64;
    let start = Instant::now();
    for _ in 0..requests {
        let mut context = runtime
            .context_builder()
            .fuel(5_000_000)
            .global("input_json", black_box(input.clone()))
            .build();
        let exports = context
            .run_module_package(package)
            .expect("normalization request executes");
        black_box(
            exports
                .get("result")
                .and_then(Value::as_str)
                .expect("normalization result is exported"),
        );
        let stats = context.last_execution();
        instructions += stats.instructions;
        managed_objects += stats.managed_objects_allocated;
        managed_bytes += stats.managed_bytes_allocated;
    }
    let elapsed = start.elapsed();
    println!(
        "normalization-scale-{event_count}: {:.3}ms for {requests} requests/{total_events} events, {:.0} events/s, {:.1}us/request, {:.0} instructions/event, {:.1} managed-objects/event, {:.1} managed-bytes/event",
        elapsed.as_secs_f64() * 1_000.,
        total_events as f64 / elapsed.as_secs_f64(),
        elapsed.as_secs_f64() * 1_000_000. / requests as f64,
        instructions as f64 / total_events as f64,
        managed_objects as f64 / total_events as f64,
        managed_bytes as f64 / total_events as f64,
    );
}

fn measure_context_cost(runtime: &Runtime) {
    let iterations = 10_000;
    let input = Value::from(INPUT);
    let start = Instant::now();
    for _ in 0..iterations {
        black_box(
            runtime
                .context_builder()
                .fuel(250_000)
                .global("input_json", input.clone())
                .build(),
        );
    }
    let elapsed = start.elapsed();
    println!(
        "normalization-context-{iterations}: {:.3}ms total, {:.1}ns/context",
        elapsed.as_secs_f64() * 1_000.,
        elapsed.as_secs_f64() * 1_000_000_000. / iterations as f64,
    );
}

fn main() {
    let start = Instant::now();
    for _ in 0..10 {
        let cold_runtime = Runtime::new();
        let cold_loader = loader();
        black_box(prepare(&cold_runtime, &cold_loader));
    }
    let elapsed = start.elapsed();
    println!(
        "normalization-cold-prepare-10: {:.3}ms total, {:.1}us/package",
        elapsed.as_secs_f64() * 1_000.,
        elapsed.as_secs_f64() * 100_000.,
    );

    let runtime = Runtime::builder()
        .program_cache_entries(16)
        .module_cache_entries(16)
        .build();
    let loader = loader();
    let start = Instant::now();
    for _ in 0..100 {
        black_box(prepare(&runtime, &loader));
    }
    let elapsed = start.elapsed();
    println!(
        "normalization-cached-prepare-100: {:.3}ms total, {:.1}us/package",
        elapsed.as_secs_f64() * 1_000.,
        elapsed.as_secs_f64() * 10_000.,
    );

    let package = prepare(&runtime, &loader);
    let mut semantic_context = runtime
        .context_builder()
        .fuel(250_000)
        .global("input_json", Value::from(INPUT))
        .build();
    let semantic = semantic_context
        .run_module_package(&package)
        .expect("normalization semantic guard executes");
    assert_eq!(
        semantic.get("result").and_then(Value::as_str),
        Some(EXPECTED.trim_end())
    );
    measure_context_cost(&runtime);
    for event_count in [10, 100, 1_000] {
        execute(&runtime, &package, event_count);
    }
}
