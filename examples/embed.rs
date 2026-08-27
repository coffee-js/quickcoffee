use quickcoffee::{CancellationToken, Error, ResourceLimits, Runtime, Value};

fn main() -> Result<(), Error> {
    let cancellation = CancellationToken::new();
    let runtime = Runtime::builder()
        .program_cache_entries(64)
        .module_cache_entries(64)
        .build();
    let mut context = runtime
        .context_builder()
        .fuel(100_000)
        .max_call_depth(128)
        .resource_limits(
            ResourceLimits::default()
                .with_max_json_input_bytes(256_000)
                .with_max_json_output_bytes(256_000)
                .with_max_integer_bits(4_096)
                .with_max_decimal_coefficient_bits(4_096)
                .with_max_decimal_scale(256),
        )
        .cancellation_token(cancellation.clone())
        .global("factor", Value::from(2_f64))
        .native("host_add", |args| {
            if args.len() != 2 {
                return Err(Error::runtime("host_add expects two numbers"));
            }
            let (Some(left), Some(right)) = (args[0].as_number(), args[1].as_number()) else {
                return Err(Error::runtime("host_add expects two numbers"));
            };
            Ok(Value::from(left + right))
        })
        .build();
    // The request owner may call `cancellation.cancel()` from another control path.
    let value = context.eval_named(
        "virtual://example/embed.coffee",
        "host_add(20, 22) * factor",
    )?;
    println!("{value}");
    let retained = context.sample_retained_memory();
    let retained_high_water = context.retained_memory_high_water();
    eprintln!(
        "retained objects={} bytes={} sampled_high_water_objects={} sampled_high_water_bytes={}",
        retained.objects, retained.bytes, retained_high_water.objects, retained_high_water.bytes,
    );
    Ok(())
}
