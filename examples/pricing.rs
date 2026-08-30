use quickcoffee::{
    Decimal, Error, ExecutionPolicy, ModulePackage, RestrictedFileModuleLoader, Runtime, Value,
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
    let mut context = runtime.context_builder().global("request", request).build();
    let exports = context.run_module_package(package)?;
    exports
        .get("result")
        .cloned()
        .ok_or_else(|| Error::runtime("pricing host module did not export result"))
}

fn main() -> Result<(), Error> {
    let runtime = Runtime::builder()
        .execution_policy(ExecutionPolicy::isolated_request())
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
