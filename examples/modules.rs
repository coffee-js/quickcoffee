use quickcoffee::{Context, Engine, Error, MemoryModuleLoader, RestrictedFileModuleLoader};
use std::path::PathBuf;

fn main() -> Result<(), Error> {
    let engine = Engine::new();
    let main = engine.compile_module(
        "main",
        "import { rate } from 'pricing'\nexport total = rate * 21",
    )?;
    let mut loader = MemoryModuleLoader::new();
    loader.insert("pricing", "export rate = 2");

    println!("{:016x}", engine.fingerprint_module_graph(&main, &loader)?);
    let package = engine.prepare_module_package(&main, &loader)?;
    let exports = Context::new().run_module_package(&package)?;
    println!("{}", exports.get("total").expect("declared module export"));

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/modules");
    let loader = RestrictedFileModuleLoader::new(root)?;
    let source = loader.load_entry("main")?;
    let main = engine.compile_module(source.name(), source.source())?;
    println!("{:016x}", engine.fingerprint_module_graph(&main, &loader)?);
    let package = engine.prepare_module_package(&main, &loader)?;
    let exports = Context::new().run_module_package(&package)?;
    println!("{}", exports.get("total").expect("declared module export"));
    Ok(())
}
