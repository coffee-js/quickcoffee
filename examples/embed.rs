use quickcoffee::{CancellationToken, Context, Error, Value};

fn main() -> Result<(), Error> {
    let cancellation = CancellationToken::new();
    let mut context = Context::new()
        .with_fuel(100_000)
        .with_max_call_depth(128)
        .with_cancellation_token(cancellation.clone())
        .with_global("factor", Value::from(2_f64))
        .with_native("host_add", |args| {
            if args.len() != 2 {
                return Err(Error::runtime("host_add expects two numbers"));
            }
            let (Some(left), Some(right)) = (args[0].as_number(), args[1].as_number()) else {
                return Err(Error::runtime("host_add expects two numbers"));
            };
            Ok(Value::from(left + right))
        });
    // The request owner may call `cancellation.cancel()` from another control path.
    let value = context.eval_named("virtual://example/embed.qc", "host_add(20, 22) * factor")?;
    println!("{value}");
    Ok(())
}
