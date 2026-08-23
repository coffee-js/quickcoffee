use quickcoffee::{Context, Error, Value};

fn main() -> Result<(), Error> {
    let mut context = Context::new()
        .with_fuel(100_000)
        .with_global("factor", Value::from(2_i64))
        .with_native("host_add", |args| {
            if args.len() != 2 {
                return Err(Error::runtime("host_add expects two numbers"));
            }
            let (Some(left), Some(right)) = (args[0].as_number(), args[1].as_number()) else {
                return Err(Error::runtime("host_add expects two numbers"));
            };
            Ok(Value::from(left + right))
        });
    let value = context.eval("host_add(20, 22) * factor")?;
    println!("{value}");
    Ok(())
}
