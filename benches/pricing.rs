//! Run with `cargo bench --bench pricing`.
//!
//! The benchmark loads the same literate rule and host module used by the CLI,
//! embedding example, and integration tests. It measures package preflight separately
//! from isolated request execution and intentionally uses no external framework.
use quickcoffee::{Decimal, ModulePackage, RestrictedFileModuleLoader, Runtime, Value};
use std::{hint::black_box, path::PathBuf, time::Instant};

fn loader() -> RestrictedFileModuleLoader {
    RestrictedFileModuleLoader::new(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/pricing"),
    )
    .expect("pricing example root is available")
}

fn prepare(runtime: &Runtime, loader: &RestrictedFileModuleLoader) -> ModulePackage {
    let source = loader.load_entry("host").expect("pricing host loads");
    let entry = runtime
        .compile_module(source.name(), source.source())
        .expect("pricing host compiles");
    runtime
        .prepare_module_package(&entry, loader)
        .expect("pricing package preflights")
}

fn request(index: usize) -> Value {
    Value::map([
        (
            "subtotal",
            Value::from(Decimal::parse(if index % 2 == 0 { "120" } else { "250.50" }).unwrap()),
        ),
        ("item_count", Value::from(3_i64)),
        (
            "customer_tier",
            Value::from(if index % 2 == 0 { "member" } else { "standard" }),
        ),
        (
            "country",
            Value::from(if index % 2 == 0 { "CN" } else { "US" }),
        ),
    ])
}

fn execute(runtime: &Runtime, package: &ModulePackage, count: usize) {
    let start = Instant::now();
    for index in 0..count {
        let mut context = runtime
            .context_builder()
            .fuel(100_000)
            .global("request", black_box(request(index)))
            .build();
        let exports = context
            .run_module_package(package)
            .expect("pricing request executes");
        black_box(exports.get("result").expect("pricing result is exported"));
    }
    let elapsed = start.elapsed();
    println!(
        "pricing-execute-{count}: {:.3}ms total, {:.0} requests/s, {:.1}us/request",
        elapsed.as_secs_f64() * 1_000.,
        count as f64 / elapsed.as_secs_f64(),
        elapsed.as_secs_f64() * 1_000_000. / count as f64,
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
        "pricing-cold-prepare-10: {:.3}ms total, {:.1}us/package",
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
        "pricing-cached-prepare-100: {:.3}ms total, {:.1}us/package",
        elapsed.as_secs_f64() * 1_000.,
        elapsed.as_secs_f64() * 10_000.,
    );

    let package = prepare(&runtime, &loader);
    for count in [10, 100, 1_000] {
        execute(&runtime, &package, count);
    }
}
