use quickcoffee::{
    CompileLimits, Error, ModulePackage, ResourceLimits, RestrictedFileModuleLoader, Runtime, Value,
};
use std::{env, fs, path::PathBuf};

fn normalize(
    runtime: &Runtime,
    package: &ModulePackage,
    input_json: String,
) -> Result<String, Error> {
    let mut context = runtime
        .context_builder()
        .fuel(250_000)
        .max_call_depth(64)
        .resource_limits(
            ResourceLimits::default()
                .with_max_string_bytes(64_000)
                .with_max_array_items(1_024)
                .with_max_map_entries(128)
                .with_max_json_input_bytes(256_000)
                .with_max_json_output_bytes(256_000)
                .with_max_json_string_bytes(64_000)
                .with_max_json_container_items(1_024)
                .with_max_json_values(16_000)
                .with_max_json_nesting_depth(32)
                .with_max_collection_operation_items(4_096)
                .with_max_integer_bits(256)
                .with_max_decimal_coefficient_bits(256)
                .with_max_decimal_scale(8)
                .with_max_transient_managed_objects(50_000)
                .with_max_transient_managed_bytes(8_000_000),
        )
        .global("input_json", Value::from(input_json))
        .build();
    let exports = context.run_module_package(package)?;
    exports
        .get("result")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| Error::runtime("normalization host module did not export a String result"))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/normalization");
    let input_path = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("input.v1.json"));
    let input_json = fs::read_to_string(input_path)?;

    let runtime = Runtime::builder()
        .compile_limits(
            CompileLimits::default()
                .with_max_source_bytes(128_000)
                .with_max_bytecode_instructions(40_000)
                .with_max_module_graph_modules(4)
                .with_max_module_graph_source_bytes(256_000),
        )
        .program_cache_entries(16)
        .module_cache_entries(16)
        .build();
    let loader = RestrictedFileModuleLoader::new(root)?;
    let source = loader.load_entry("host")?;
    let entry = runtime.compile_module(source.name(), source.source())?;
    let package = runtime.prepare_module_package(&entry, &loader)?;

    println!("{}", normalize(&runtime, &package, input_json)?);
    Ok(())
}
