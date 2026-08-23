use quickcoffee::{
    CancellationToken, Context, Engine, Error, ErrorKind, ResourceLimit, Value, ValueKind,
};

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
    assert_eq!(error.kind(), ErrorKind::Resource);
    assert_eq!(error.resource_limit(), Some(ResourceLimit::Fuel));
    assert!(error.message().contains("fuel exhausted"));
    let stats = context.last_execution();
    assert_eq!(stats.instructions, 8);
    assert_eq!(stats.fuel_remaining, 0);
    assert_eq!(stats.call_depth_peak, 0);
}

#[test]
fn execution_stats_classify_hot_vm_operations() {
    let mut context = Context::new();
    assert_eq!(
        context.eval("value = 1\nvalue").unwrap().as_number(),
        Some(1.)
    );
    let stats = context.last_execution();
    assert_eq!(stats.name_loads, 1);
    assert_eq!(stats.name_stores, 1);
    assert_eq!(stats.calls, 0);

    assert_eq!(
        context
            .eval("item = 1\nlen([item, 2])")
            .unwrap()
            .as_number(),
        Some(2.)
    );
    let stats = context.last_execution();
    assert_eq!(stats.calls, 1);
    assert_eq!(stats.container_ops, 1);

    assert_eq!(
        context
            .eval("sum = 0\nfor value in [1...3] then sum += value\nsum")
            .unwrap()
            .as_number(),
        Some(3.)
    );
    assert!(context.last_execution().iterator_ops >= 4);

    assert_eq!(
        context
            .eval("try throw 1 catch error then 42")
            .unwrap()
            .as_number(),
        Some(42.)
    );
    assert!(context.last_execution().exception_ops >= 2);
    assert!(context.eval("throw 1").is_err());
    assert_eq!(context.last_execution().exception_ops, 1);
}

#[test]
fn execution_stats_count_managed_value_and_environment_allocations() {
    let mut context = Context::new();
    assert_eq!(
        context
            .eval("make = (value) -> [value]\nmake(42)")
            .unwrap()
            .as_array()
            .and_then(|values| values[0].as_number()),
        Some(42.)
    );
    let stats = context.last_execution();
    assert_eq!(stats.value_allocations, 2);
    assert_eq!(stats.environment_allocations, 1);

    context.add_native("host_array", |_| Ok(Value::array(vec![Value::from(1_i64)])));
    assert_eq!(
        context
            .eval("host_array()")
            .unwrap()
            .as_array()
            .map(<[Value]>::len),
        Some(1)
    );
    let stats = context.last_execution();
    assert_eq!(stats.value_allocations, 0);
    assert_eq!(stats.environment_allocations, 0);

    assert_eq!(
        context
            .eval("keys({a: 1, b: 2})")
            .unwrap()
            .as_array()
            .map(<[Value]>::len),
        Some(2)
    );
    assert_eq!(context.last_execution().value_allocations, 3);
}

#[test]
fn resource_limits_bound_call_depth_and_cannot_be_caught_by_scripts() {
    let mut context = Context::new().with_fuel(1_000).with_max_call_depth(3);
    assert_eq!(context.max_call_depth(), 3);
    let error = context
        .eval("recur = -> recur()\ntry recur() catch ignored then 42")
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Resource);
    assert_eq!(error.resource_limit(), Some(ResourceLimit::CallDepth));
    assert!(error.message().contains("call depth of 3"));
    let stats = context.last_execution();
    assert_eq!(stats.call_depth_peak, 3);
    assert!(stats.fuel_remaining > 0);

    context.set_max_call_depth(0);
    assert_eq!(context.max_call_depth(), 0);
    let error = context.eval("(-> 1)()").unwrap_err();
    assert_eq!(error.resource_limit(), Some(ResourceLimit::CallDepth));
}

#[test]
fn cancellation_token_stops_runs_before_execution_and_can_be_replaced() {
    let token = CancellationToken::new();
    let mut context = Context::new()
        .with_fuel(20)
        .with_cancellation_token(token.clone());
    token.cancel();
    assert!(token.is_cancelled());
    let error = context.eval("1 + 2").unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Resource);
    assert_eq!(error.resource_limit(), Some(ResourceLimit::Cancellation));
    assert_eq!(context.last_execution().instructions, 0);
    assert_eq!(context.last_execution().fuel_remaining, 20);

    let replacement = CancellationToken::new();
    context.set_cancellation_token(replacement.clone());
    assert_eq!(context.eval("1 + 2").unwrap().as_number(), Some(3.));

    replacement.cancel();
    let error = context.eval("1 + 2").unwrap_err();
    assert_eq!(error.resource_limit(), Some(ResourceLimit::Cancellation));

    context.clear_cancellation_token();
    assert_eq!(context.eval("1 + 2").unwrap().as_number(), Some(3.));
}
