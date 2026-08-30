use quickcoffee::{
    Error, ExecutionPolicy, ModulePackage, RestrictedFileModuleLoader, Runtime, Value,
};
use std::{env, fs, path::PathBuf};

fn normalize(
    runtime: &Runtime,
    package: &ModulePackage,
    input_json: String,
) -> Result<String, Error> {
    let mut context = runtime
        .context_builder()
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
        .execution_policy(ExecutionPolicy::isolated_request())
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
