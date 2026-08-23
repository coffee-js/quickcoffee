use quickcoffee::{Context, Engine, Error, ErrorKind, Value, ValueKind};

#[test]
fn public_embedding_surface_runs_shared_programs_with_host_state() {
    let engine = Engine::new();
    let program = engine
        .compile_program("host(20, 22) * factor")
        .expect("public Engine API compiles a shared program");
    let clone = program.clone();
    assert_eq!(program.fingerprint(), clone.fingerprint());

    let mut context = Context::new();
    context.set_global("factor", Value::from(2_i64));
    context.add_native("host", |args| {
        let (Some(left), Some(right)) = (
            args.first().and_then(Value::as_number),
            args.get(1).and_then(Value::as_number),
        ) else {
            return Err(Error::runtime("host expects two numbers"));
        };
        Ok(Value::from(left + right))
    });

    assert_eq!(
        context.run_program(&program).unwrap().as_number(),
        Some(84.)
    );
    assert_eq!(context.run_program(&clone).unwrap().as_number(), Some(84.));
    assert_eq!(context.get_global("factor").unwrap().as_number(), Some(2.));
    assert!(context.get_global("missing").is_none());
}

#[test]
fn builder_embedding_surface_chains_host_configuration() {
    let program = Engine::new()
        .compile_program("host(20, 22) * factor")
        .unwrap();
    let mut context = Context::new()
        .with_global("factor", Value::from(2_i64))
        .with_native("host", |args| {
            let (Some(left), Some(right)) = (
                args.first().and_then(Value::as_number),
                args.get(1).and_then(Value::as_number),
            ) else {
                return Err(Error::runtime("host expects two numbers"));
            };
            Ok(Value::from(left + right))
        });
    assert_eq!(
        context.run_program(&program).unwrap().as_number(),
        Some(84.)
    );
    assert_eq!(context.get_global("factor").unwrap().as_number(), Some(2.));
}

#[test]
fn public_values_and_native_errors_are_structured() {
    let mut context = Context::new();
    context.set_global(
        "host_values",
        Value::map([
            ("answer", Value::from(42_i64)),
            ("items", Value::array(vec![Value::from("coffee")])),
        ]),
    );
    let values = context.get_global("host_values").unwrap();
    assert_eq!(values.kind(), ValueKind::Map);
    assert!(!values.is_nil());
    let map = values.as_map().unwrap();
    assert_eq!(map["answer"].as_number(), Some(42.));
    assert_eq!(map["items"].as_array().unwrap()[0].as_str(), Some("coffee"));
    assert_eq!(map["items"].kind(), ValueKind::Array);
    assert_eq!(Value::Nil.kind(), ValueKind::Nil);
    assert!(Value::Nil.is_nil());

    context.add_native("fail", |_| Err(Error::runtime("host failed")));
    let error = context.eval("fail()").unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Runtime);
    assert_eq!(error.message(), "host failed");
}

#[test]
fn public_fuel_and_execution_stats_bound_untrusted_programs() {
    let mut context = Context::new().with_fuel(8);
    let error = context.eval("while true then 1").unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Runtime);
    assert!(error.message().contains("fuel exhausted"));
    let stats = context.last_execution();
    assert_eq!(stats.instructions, 8);
    assert_eq!(stats.fuel_remaining, 0);
}
