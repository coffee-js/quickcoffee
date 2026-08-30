use quickcoffee::{
    Decimal, Error, ExecutionPolicy, ModulePackage, RestrictedFileModuleLoader, Runtime, Value,
    parse_cson,
};
use std::{collections::BTreeMap, env, fs, io, path::PathBuf};

fn invalid_config(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn required_string<'a>(
    map: &'a BTreeMap<String, Value>,
    section: &str,
    field: &str,
) -> Result<&'a str, io::Error> {
    map.get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_config(format!("{section}.{field} must be a string")))
}

fn configured_order(config: &Value, section: &str) -> Result<Value, io::Error> {
    let root = config
        .as_map()
        .ok_or_else(|| invalid_config("pricing configuration must be a map"))?;
    if root.get("schema").and_then(Value::as_str) != Some("pricing-orders/v1") {
        return Err(invalid_config("schema must be pricing-orders/v1"));
    }
    let order = root
        .get(section)
        .and_then(Value::as_map)
        .ok_or_else(|| invalid_config(format!("{section} must be a map")))?;
    let subtotal_source = required_string(order, section, "subtotal")?;
    let subtotal = Decimal::parse(subtotal_source)
        .ok_or_else(|| invalid_config(format!("{section}.subtotal must be a decimal string")))?;
    let item_count = order
        .get("item_count")
        .and_then(Value::as_integer)
        .and_then(|value| value.as_i64())
        .ok_or_else(|| invalid_config(format!("{section}.item_count must fit i64")))?;
    Ok(Value::map([
        ("subtotal", Value::from(subtotal)),
        ("item_count", Value::from(item_count)),
        (
            "customer_tier",
            Value::from(required_string(order, section, "customer_tier")?),
        ),
        (
            "country",
            Value::from(required_string(order, section, "country")?),
        ),
    ]))
}

fn quote(runtime: &Runtime, package: &ModulePackage, request: Value) -> Result<Value, Error> {
    let mut context = runtime.context_builder().global("request", request).build();
    let exports = context.run_module_package(package)?;
    exports
        .get("result")
        .cloned()
        .ok_or_else(|| Error::runtime("pricing host module did not export result"))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/pricing");
    let config_path = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("config.cson"));
    let config_source = fs::read_to_string(config_path)?;
    let config = parse_cson(&config_source)?;

    let runtime = Runtime::builder()
        .execution_policy(ExecutionPolicy::isolated_request())
        .program_cache_entries(16)
        .module_cache_entries(16)
        .build();
    let loader = RestrictedFileModuleLoader::new(root)?;
    let source = loader.load_entry("host")?;
    let entry = runtime.compile_module(source.name(), source.source())?;
    let package = runtime.prepare_module_package(&entry, &loader)?;

    let accepted = quote(&runtime, &package, configured_order(&config, "accepted")?)?;

    let rejected = configured_order(&config, "rejected")?;
    let error = quote(&runtime, &package, rejected).expect_err("order should be ineligible");
    let code = error
        .script_error()
        .map(|error| error.code())
        .unwrap_or("runtime");
    println!(
        "{}",
        Value::map([("quote", accepted), ("rejection", Value::from(code))])
    );
    Ok(())
}
