use quickcoffee::{
    CompileLimits, Decimal, Error, ModulePackage, ResourceLimits, RestrictedFileModuleLoader,
    Runtime, Value,
};
use std::path::PathBuf;

fn decimal(source: &str) -> Value {
    Value::from(Decimal::parse(source).expect("valid example decimal"))
}

fn order(subtotal: &str, item_count: i64, customer_tier: &str, country: &str) -> Value {
    Value::map([
        ("subtotal", decimal(subtotal)),
        ("item_count", Value::from(item_count)),
        ("customer_tier", Value::from(customer_tier)),
        ("country", Value::from(country)),
    ])
}

fn quote(runtime: &Runtime, package: &ModulePackage, request: Value) -> Result<Value, Error> {
    let mut context = runtime
        .context_builder()
        .fuel(100_000)
        .max_call_depth(64)
        .resource_limits(
            ResourceLimits::default()
                .with_max_map_entries(32)
                .with_max_array_items(32)
                .with_max_string_bytes(4_096)
                .with_max_decimal_coefficient_bits(256)
                .with_max_decimal_scale(8)
                .with_max_transient_managed_objects(10_000)
                .with_max_transient_managed_bytes(1_000_000),
        )
        .global("request", request)
        .build();
    let exports = context.run_module_package(package)?;
    exports
        .get("result")
        .cloned()
        .ok_or_else(|| Error::runtime("pricing host module did not export result"))
}

fn main() -> Result<(), Error> {
    let runtime = Runtime::builder()
        .compile_limits(
            CompileLimits::default()
                .with_max_source_bytes(64_000)
                .with_max_bytecode_instructions(20_000)
                .with_max_module_graph_modules(4)
                .with_max_module_graph_source_bytes(128_000),
        )
        .program_cache_entries(16)
        .module_cache_entries(16)
        .build();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/pricing");
    let loader = RestrictedFileModuleLoader::new(root)?;
    let source = loader.load_entry("host")?;
    let entry = runtime.compile_module(source.name(), source.source())?;
    let package = runtime.prepare_module_package(&entry, &loader)?;

    let accepted = order("120", 3, "member", "CN");
    println!("accepted: {}", quote(&runtime, &package, accepted)?);

    let rejected = order("5", 1, "standard", "US");
    let error = quote(&runtime, &package, rejected).expect_err("order should be ineligible");
    let code = error
        .script_error()
        .map(|error| error.code())
        .unwrap_or("runtime");
    println!("rejected: {code}");
    Ok(())
}
