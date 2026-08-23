use quickcoffee::{Context, Engine, Error, MemoryModuleLoader};

fn main() -> Result<(), Error> {
    let main = Engine::new().compile_module(
        "main",
        "import { rate } from 'pricing'\nexport total = rate * 21",
    )?;
    let mut loader = MemoryModuleLoader::new();
    loader.insert("pricing", "export rate = 2");

    let exports = Context::new().run_module(&main, &loader)?;
    println!("{}", exports.get("total").expect("declared module export"));
    Ok(())
}
