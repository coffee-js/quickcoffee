use quickcoffee::{Context, Engine, Error, MemoryModuleLoader, RestrictedFileModuleLoader};
use std::path::PathBuf;

fn main() -> Result<(), Error> {
    let main = Engine::new().compile_module(
        "main",
        "import { rate } from 'pricing'\nexport total = rate * 21",
    )?;
    let mut loader = MemoryModuleLoader::new();
    loader.insert("pricing", "export rate = 2");

    let exports = Context::new().run_module(&main, &loader)?;
    println!("{}", exports.get("total").expect("declared module export"));

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/modules");
    let loader = RestrictedFileModuleLoader::new(root)?;
    let source = loader.load_entry("main")?;
    let main = Engine::new().compile_module(source.name(), source.source())?;
    let exports = Context::new().run_module(&main, &loader)?;
    println!("{}", exports.get("total").expect("declared module export"));
    Ok(())
}
