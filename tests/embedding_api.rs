use quickcoffee::{
    CancellationToken, CapabilityKey, CapabilityKind, Chunk, CompileLimits, Constant, Context,
    Decimal, DiagnosticLabelKind, Engine, Error, ErrorKind, ExecutionStats, HostCapabilities,
    Instruction, Integer, IntoValue, Program, ResourceLimit, ResourceLimits, RetainedMemory,
    Runtime, TryFromValue, Value, ValueKind,
};
use std::{cell::Cell, collections::BTreeMap};

#[test]
fn public_embedding_surface_runs_shared_programs_with_host_state() {
    let engine = Engine::new();
    let program = engine
        .compile_program("host(20, 22) * factor")
        .expect("public Engine API compiles a shared program");
    let clone = program.clone();
    assert_eq!(program.fingerprint(), clone.fingerprint());

    let mut context = Context::new();
    context.set_global("factor", Value::from(2_f64));
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
fn runtime_context_builders_share_compilation_but_isolate_execution_state() {
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    let limits = ResourceLimits::default().with_max_string_bytes(128);
    let runtime = Runtime::builder()
        .program_cache_entries(4)
        .module_cache_entries(0)
        .build();
    let mut first = runtime
        .context_builder()
        .fuel(20)
        .max_call_depth(7)
        .resource_limits(limits)
        .cancellation_token(cancelled)
        .global("factor", Value::from(2_f64))
        .native("host", |_| Ok(Value::from(20_f64)))
        .build();
    let mut second = runtime
        .context_builder()
        .fuel(40)
        .global("factor", Value::from(3_f64))
        .native("host", |_| Ok(Value::from(14_f64)))
        .build();

    let error = first
        .eval_named("shared.coffee", "host() * factor")
        .unwrap_err();
    assert_eq!(error.resource_limit(), Some(ResourceLimit::Cancellation));
    assert_eq!(first.fuel(), 20);
    assert_eq!(second.fuel(), 40);
    assert_eq!(first.max_call_depth(), 7);
    assert_eq!(second.max_call_depth(), 1_024);
    assert_eq!(first.resource_limits().max_string_bytes(), 128);
    assert_ne!(second.resource_limits().max_string_bytes(), 128);
    assert_eq!(
        second
            .eval_named("shared.coffee", "host() * factor")
            .unwrap()
            .as_number(),
        Some(42.)
    );
    second.eval("private = 'second'").unwrap();
    assert!(first.get_global("private").is_none());
    assert_eq!(first.get_global("factor").unwrap().as_number(), Some(2.));
    assert_eq!(second.get_global("factor").unwrap().as_number(), Some(3.));
    assert_eq!(first.last_execution().instructions, 0);
    assert!(second.last_execution().instructions > 0);
    assert_eq!(first.retained_memory().objects, 2);
    assert!(second.retained_memory().objects > first.retained_memory().objects);

    let stats = runtime.cache_stats();
    assert_eq!(stats.program_entries, 2);
    assert_eq!(stats.program_hits, 1);
    assert_eq!(stats.program_misses, 2);
    assert_eq!(stats.module_entries, 0);
}

#[test]
fn runtime_program_cache_is_exact_bounded_clearable_and_ignores_failures() {
    let runtime = Runtime::builder()
        .program_cache_entries(1)
        .module_cache_entries(0)
        .build();
    runtime
        .compile_program_named("one.coffee", "40 + 2")
        .unwrap();
    runtime
        .compile_program_named("one.coffee", "40 + 2")
        .unwrap();
    runtime
        .compile_program_named("one.coffee", "41 + 1")
        .unwrap();
    assert!(runtime.compile_program_named("bad.coffee", "if").is_err());
    runtime
        .compile_program_named("two.coffee", "41 + 1")
        .unwrap();
    let before_clear = runtime.cache_stats();
    assert_eq!(before_clear.program_entries, 1);
    assert_eq!(before_clear.program_hits, 1);
    assert_eq!(before_clear.program_misses, 4);
    assert_eq!(before_clear.program_evictions, 2);

    runtime.clear_compile_caches();
    assert_eq!(runtime.cache_stats().program_entries, 0);
    runtime
        .compile_program_named("one.coffee", "41 + 1")
        .unwrap();
    let after_clear = runtime.cache_stats();
    assert_eq!(after_clear.program_misses, 5);
    assert_eq!(after_clear.program_entries, 1);

    let lru = Runtime::builder()
        .program_cache_entries(2)
        .module_cache_entries(0)
        .build();
    lru.compile_program_named("a.coffee", "40 + 2").unwrap();
    lru.compile_program_named("b.coffee", "40 + 2").unwrap();
    lru.compile_program_named("a.coffee", "40 + 2").unwrap();
    lru.compile_program_named("c.coffee", "40 + 2").unwrap();
    lru.compile_program_named("a.coffee", "40 + 2").unwrap();
    assert_eq!(lru.cache_stats().program_hits, 2);
    assert_eq!(lru.cache_stats().program_misses, 3);
    assert_eq!(lru.cache_stats().program_evictions, 1);

    let disabled = Runtime::builder()
        .program_cache_entries(0)
        .module_cache_entries(0)
        .build();
    disabled.compile_program("42").unwrap();
    disabled.compile_program("42").unwrap();
    assert_eq!(disabled.cache_stats().program_entries, 0);
    assert_eq!(disabled.cache_stats().program_hits, 0);
    assert_eq!(disabled.cache_stats().program_misses, 2);
}

#[test]
fn context_builder_applies_mixed_bindings_in_declaration_order() {
    let mut direct = Context::new();
    direct.add_native("configured", |_| Ok(Value::from(1_f64)));
    direct.set_global("configured", Value::from(2_f64));
    assert_eq!(
        direct.get_global("configured").unwrap().as_number(),
        Some(2.)
    );

    let runtime = Runtime::new();
    let mut global_wins = runtime
        .context_builder()
        .native("configured", |_| Ok(Value::from(1_f64)))
        .global("configured", Value::from(2_f64))
        .build();
    assert_eq!(
        global_wins.get_global("configured").unwrap().as_number(),
        Some(2.)
    );
    assert_eq!(
        global_wins.eval("configured").unwrap().as_number(),
        Some(2.)
    );

    let mut native_wins = runtime
        .context_builder()
        .global("configured", Value::from(2_f64))
        .native("configured", |_| Ok(Value::from(3_f64)))
        .build();
    assert_eq!(
        native_wins.eval("configured()").unwrap().as_number(),
        Some(3.)
    );
}

#[test]
fn contextual_natives_cooperate_with_fuel_limits_telemetry_and_typed_state() {
    let limits = ResourceLimits::default().with_max_string_bytes(128);
    let runtime = Runtime::new();
    let mut context = runtime
        .context_builder()
        .fuel(100)
        .resource_limits(limits)
        .host_state(Cell::new(40_u64))
        .contextual_native("host_step", |call, args| {
            assert!(args.is_empty());
            assert_eq!(call.resource_limits().max_string_bytes(), 128);
            assert!(call.fuel_remaining() < 100);
            assert!(!call.is_cancelled());
            assert!(call.host_state::<String>().is_none());
            let counter = call
                .host_state::<Cell<u64>>()
                .ok_or_else(|| Error::runtime("missing counter state"))?;
            counter.set(counter.get() + 1);
            call.consume_fuel(5)?;
            call.record_managed_allocation(2, 7);
            Ok(Value::from(counter.get() as f64))
        })
        .build();

    let before = context.retained_memory();
    assert_eq!(context.eval("host_step()").unwrap().as_number(), Some(41.));
    assert_eq!(context.host_state::<Cell<u64>>().unwrap().get(), 41);
    assert_eq!(context.retained_memory(), before);
    let stats = context.last_execution();
    assert_eq!(stats.fuel_remaining, 100 - stats.instructions - 5);
    assert_eq!(stats.managed_objects_allocated, 2);
    assert_eq!(stats.managed_bytes_allocated, 7);
    assert_eq!(stats.value_allocations, 0);

    context.set_host_state(String::from("replacement"));
    assert!(context.host_state::<Cell<u64>>().is_none());
    assert_eq!(
        context.host_state::<String>().map(String::as_str),
        Some("replacement")
    );
    context.clear_host_state();
    assert!(context.host_state::<String>().is_none());
}

#[test]
fn contextual_native_resource_stops_are_uncatchable_labeled_and_account_failures() {
    let token = CancellationToken::new();
    let callback_token = token.clone();
    let mut cancelled = Context::new()
        .with_cancellation_token(token)
        .with_contextual_native("cancel_host", move |call, _| {
            callback_token.cancel();
            call.check_cancelled()?;
            Ok(Value::Nil)
        });
    let error = cancelled
        .eval_named(
            "native-cancel.coffee",
            "try cancel_host() catch problem then 42",
        )
        .unwrap_err();
    assert_eq!(error.resource_limit(), Some(ResourceLimit::Cancellation));
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("native-cancel.coffee")
    );

    let mut exhausted =
        Context::new()
            .with_fuel(50)
            .with_contextual_native("host_work", |call, _| {
                call.record_managed_allocation(4, 9);
                call.consume_fuel(u64::MAX)?;
                Ok(Value::Nil)
            });
    let error = exhausted
        .eval("try host_work() catch problem then 42")
        .unwrap_err();
    assert_eq!(error.resource_limit(), Some(ResourceLimit::Fuel));
    let stats = exhausted.last_execution();
    assert_eq!(stats.fuel_remaining, 0);
    assert_eq!(stats.managed_objects_allocated, 4);
    assert_eq!(stats.managed_bytes_allocated, 9);

    let mut failed = Context::new()
        .with_fuel(50)
        .with_contextual_native("host_fail", |call, _| {
            call.consume_fuel(3)?;
            call.record_managed_allocation(6, 11);
            Err(Error::runtime("contextual host failed"))
        });
    let error = failed.eval("host_fail()").unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Runtime);
    assert_eq!(error.message(), "contextual host failed");
    let stats = failed.last_execution();
    assert_eq!(stats.fuel_remaining, 50 - stats.instructions - 3);
    assert_eq!(stats.managed_objects_allocated, 6);
    assert_eq!(stats.managed_bytes_allocated, 11);
}

#[test]
fn runtime_contexts_do_not_share_host_state_implicitly() {
    let runtime = Runtime::new();
    let make_context = |initial| {
        runtime
            .context_builder()
            .host_state(Cell::new(initial))
            .contextual_native("state", |call, _| {
                let state = call
                    .host_state::<Cell<u64>>()
                    .ok_or_else(|| Error::runtime("missing state"))?;
                state.set(state.get() + 1);
                Ok(Value::from(state.get() as f64))
            })
            .build()
    };
    let mut first = make_context(10_u64);
    let mut second = make_context(20_u64);
    assert_eq!(first.eval("state()").unwrap().as_number(), Some(11.));
    assert_eq!(second.eval("state()").unwrap().as_number(), Some(21.));
    assert_eq!(first.host_state::<Cell<u64>>().unwrap().get(), 11);
    assert_eq!(second.host_state::<Cell<u64>>().unwrap().get(), 21);
}

#[test]
fn compile_limits_bound_raw_source_recursive_bytecode_and_foreign_programs() {
    let defaults = CompileLimits::default();
    assert_eq!(defaults.max_source_bytes(), 1_000_000);
    assert_eq!(defaults.max_bytecode_instructions(), 1_000_000);
    assert_eq!(defaults.max_module_graph_modules(), 1_024);
    assert_eq!(defaults.max_module_graph_source_bytes(), 16_000_000);

    let source = "f = (fallback = -> 1) -> fallback()\nf()";
    let ordinary = Engine::new().compile_program(source).unwrap();
    assert!(ordinary.instruction_count() > 2);
    let source_limited = CompileLimits::default().with_max_source_bytes(source.len() - 1);
    let error = Engine::new()
        .with_compile_limits(source_limited)
        .compile_program_named("policy.coffee", source)
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Resource);
    assert_eq!(error.resource_limit(), Some(ResourceLimit::SourceBytes));
    assert_eq!(error.position().unwrap().line, 1);
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("policy.coffee")
    );

    let bytecode_limited =
        CompileLimits::default().with_max_bytecode_instructions(ordinary.instruction_count() - 1);
    let error = Engine::new()
        .with_compile_limits(bytecode_limited)
        .compile_program_named("policy.coffee", source)
        .unwrap_err();
    assert_eq!(
        error.resource_limit(),
        Some(ResourceLimit::BytecodeInstructions)
    );
    assert_eq!(error.position().unwrap().line, 1);

    let literate = "A prose line with `inline_code`.\n\n    true";
    let error = Engine::new()
        .with_compile_limits(CompileLimits::default().with_max_source_bytes(literate.len() - 1))
        .compile_program_named("policy.litcoffee", literate)
        .unwrap_err();
    assert_eq!(error.resource_limit(), Some(ResourceLimit::SourceBytes));

    let runtime = Runtime::builder()
        .compile_limits(CompileLimits::default().with_max_source_bytes(4))
        .build();
    assert_eq!(runtime.compile_limits().max_source_bytes(), 4);
    assert_eq!(
        runtime
            .compile_program("1 + 1")
            .unwrap_err()
            .resource_limit(),
        Some(ResourceLimit::SourceBytes)
    );
    assert_eq!(runtime.cache_stats().program_misses, 0);
    runtime.compile_program("true").unwrap();
    assert_eq!(runtime.cache_stats().program_misses, 1);

    let raw = Program::from(Chunk {
        constants: vec![Constant::Value(Value::from(1_f64))],
        code: vec![Instruction::Constant(0), Instruction::Return],
    });
    assert_eq!(raw.instruction_count(), 2);
    let runtime = Runtime::builder()
        .compile_limits(CompileLimits::default().with_max_bytecode_instructions(1))
        .build();
    let mut context = runtime.new_context();
    let error = context.run_program(&raw).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Resource);
    assert_eq!(
        error.resource_limit(),
        Some(ResourceLimit::BytecodeInstructions)
    );
    assert_eq!(context.last_execution(), ExecutionStats::default());
}

#[test]
fn typed_capability_allowlists_are_explicit_isolated_and_unretained() {
    let audit = CapabilityKey::<Cell<u64>>::new(CapabilityKind::Logging, "audit");
    let wrong_audit = CapabilityKey::<String>::new(CapabilityKind::Logging, "audit");
    let clock = CapabilityKey::<u64>::new(CapabilityKind::Clock, "request-time");
    assert_eq!(audit.kind(), CapabilityKind::Logging);
    assert_eq!(audit.name(), "audit");

    let mut capabilities = HostCapabilities::new();
    assert!(capabilities.is_empty());
    capabilities.insert(audit, Cell::new(1));
    assert_eq!(capabilities.len(), 1);
    assert_eq!(
        capabilities.descriptors().collect::<Vec<_>>(),
        vec![(CapabilityKind::Logging, "audit")]
    );
    assert!(capabilities.contains(audit));
    assert!(!capabilities.contains(wrong_audit));
    assert!(!capabilities.remove(wrong_audit));
    let original_audit = capabilities.get(audit).unwrap();

    let runtime = Runtime::new();
    let mut context = runtime
        .context_builder()
        .fuel(100)
        .capabilities(capabilities.clone())
        .capability(clock, 7_u64)
        .contextual_native("host_audit", move |call, args| {
            assert!(args.is_empty());
            call.check_cancelled()?;
            call.consume_fuel(2)?;
            assert!(call.capability(wrong_audit).is_none());
            let sink = call
                .capability(audit)
                .ok_or_else(|| Error::runtime("logging capability denied"))?;
            let time = call
                .capability(clock)
                .ok_or_else(|| Error::runtime("clock capability denied"))?;
            sink.set(sink.get() + 1);
            call.record_managed_allocation(1, 2);
            Ok(Value::from((sink.get() + *time) as f64))
        })
        .build();

    let retained = context.retained_memory();
    assert_eq!(context.eval("host_audit()").unwrap().as_number(), Some(9.));
    assert_eq!(original_audit.get(), 2);
    assert_eq!(context.capability(clock).as_deref(), Some(&7));
    assert!(context.capability(wrong_audit).is_none());
    assert!(context.get_global("audit").is_none());
    assert_eq!(context.last_execution().managed_objects_allocated, 1);
    assert_eq!(context.last_execution().managed_bytes_allocated, 2);
    assert_eq!(context.retained_memory(), retained);

    context.set_capability(audit, Cell::new(40));
    assert_eq!(original_audit.get(), 2);
    assert_eq!(context.capability(audit).unwrap().get(), 40);
    assert!(context.remove_capability(clock));
    assert!(!context.remove_capability(clock));
    assert_eq!(context.capabilities().len(), 1);
    assert_eq!(context.retained_memory(), retained);
    context.clear_capabilities();
    assert!(context.capabilities().is_empty());

    let independent = runtime.context_builder().build();
    assert!(independent.capabilities().is_empty());
    assert!(independent.capability(audit).is_none());

    let mut denied = Context::new().with_contextual_native("host_audit", move |call, _| {
        call.capability(audit)
            .map(|_| Value::Nil)
            .ok_or_else(|| Error::runtime("logging capability denied"))
    });
    let error = denied.eval("host_audit()").unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Runtime);
    assert_eq!(error.message(), "logging capability denied");
}

#[test]
fn retained_memory_census_is_context_owned_cycle_safe_and_alias_aware() {
    let mut context = Context::new();
    assert_eq!(
        context.retained_memory(),
        RetainedMemory {
            objects: 1,
            bytes: 0,
        }
    );

    let shared = Value::array(vec![Value::from("bean")]);
    context.set_global("first", shared.clone());
    context.set_global("second", shared);
    assert_eq!(
        context.retained_memory(),
        RetainedMemory {
            objects: 3,
            bytes: 12,
        }
    );

    context.eval("cycle = -> cycle").unwrap();
    let cycle_snapshot = context.retained_memory();
    assert_eq!(cycle_snapshot, context.retained_memory());
    assert_eq!(cycle_snapshot.objects, 4);
    assert_eq!(cycle_snapshot.bytes, 20);

    context.set_global("first", Value::from(1_f64));
    context.set_global("second", Value::Nil);
    assert_eq!(
        context.retained_memory(),
        RetainedMemory {
            objects: 2,
            bytes: 8,
        }
    );

    context
        .eval("class Box\n  constructor: (@value) ->\nbox = new Box('coffee')")
        .unwrap();
    let object_snapshot = context.retained_memory();
    assert!(object_snapshot.objects > 2);
    assert!(object_snapshot.bytes > 8);
    assert_eq!(object_snapshot, context.retained_memory());

    let isolated = Context::new();
    assert_eq!(
        isolated.retained_memory(),
        RetainedMemory {
            objects: 1,
            bytes: 0,
        }
    );
}

#[test]
fn retained_memory_high_water_tracks_only_explicit_samples() {
    let mut context = Context::new();
    assert_eq!(
        context.retained_memory_high_water(),
        RetainedMemory {
            objects: 1,
            bytes: 0,
        }
    );

    context.set_global("payload", Value::array(vec![Value::from("coffee")]));
    assert_eq!(
        context.sample_retained_memory(),
        RetainedMemory {
            objects: 3,
            bytes: 14,
        }
    );
    context.set_global("payload", Value::Nil);
    assert_eq!(
        context.retained_memory(),
        RetainedMemory {
            objects: 1,
            bytes: 0,
        }
    );
    assert_eq!(
        context.retained_memory_high_water(),
        RetainedMemory {
            objects: 3,
            bytes: 14,
        }
    );

    context
        .eval("temporary = ['coffee', 'beans', 'espresso']; temporary = nil")
        .unwrap();
    assert_eq!(
        context.sample_retained_memory(),
        RetainedMemory {
            objects: 1,
            bytes: 0,
        }
    );
    assert_eq!(
        context.retained_memory_high_water(),
        RetainedMemory {
            objects: 3,
            bytes: 14,
        }
    );

    context.eval("kept = ['coffee', 'beans']").unwrap();
    assert_eq!(
        context.sample_retained_memory(),
        RetainedMemory {
            objects: 4,
            bytes: 27,
        }
    );
    assert!(context.eval("kept = ['espresso']; unknown()").is_err());
    assert_eq!(
        context.sample_retained_memory(),
        RetainedMemory {
            objects: 3,
            bytes: 16,
        }
    );
    assert_eq!(
        context.retained_memory_high_water(),
        RetainedMemory {
            objects: 4,
            bytes: 27,
        }
    );
}

#[test]
fn retained_memory_limits_preflight_and_roll_back_context_mutations() {
    let mut preflight = Context::new()
        .with_resource_limits(ResourceLimits::default().with_max_retained_managed_objects(1));
    preflight.set_global("host_value", Value::from("coffee"));
    let error = preflight.eval("1").unwrap_err();
    assert_eq!(
        error.resource_limit(),
        Some(ResourceLimit::RetainedManagedObjects)
    );
    assert_eq!(preflight.last_execution().instructions, 0);
    assert_eq!(
        preflight
            .get_global("host_value")
            .and_then(|value| value.as_str().map(str::to_owned)),
        Some("coffee".to_owned())
    );

    let mut context = Context::new()
        .with_resource_limits(ResourceLimits::default().with_max_retained_managed_objects(2));
    context.set_global("stable", Value::from("old"));
    let error = context.eval("next = ['coffee']").unwrap_err();
    assert_eq!(
        error.resource_limit(),
        Some(ResourceLimit::RetainedManagedObjects)
    );
    assert_eq!(
        context
            .get_global("stable")
            .and_then(|value| value.as_str().map(str::to_owned)),
        Some("old".to_owned())
    );
    assert!(context.get_global("next").is_none());
    assert!(context.last_execution().instructions > 0);

    context.set_resource_limits(ResourceLimits::default().with_max_retained_managed_bytes(3));
    let error = context.eval("stable = 'coffee'").unwrap_err();
    assert_eq!(
        error.resource_limit(),
        Some(ResourceLimit::RetainedManagedBytes)
    );
    assert_eq!(
        context
            .get_global("stable")
            .and_then(|value| value.as_str().map(str::to_owned)),
        Some("old".to_owned())
    );

    context.set_resource_limits(ResourceLimits::default());

    context
        .eval("class Box\n  constructor: (@value) ->\n  set: (value) -> @value = value\nbox = new Box('small')")
        .unwrap();
    let limit = context.retained_memory().bytes;
    context.set_resource_limits(ResourceLimits::default().with_max_retained_managed_bytes(limit));
    let error = context
        .eval("box.set('a substantially larger value')")
        .unwrap_err();
    assert_eq!(
        error.resource_limit(),
        Some(ResourceLimit::RetainedManagedBytes)
    );
    assert_eq!(context.eval("box.value").unwrap().as_str(), Some("small"));
}

#[test]
fn strict_host_value_conversions_are_recursive_and_non_coercing() {
    let input = BTreeMap::from([(
        "names".to_owned(),
        vec![Some("coffee".to_owned()), None, Some("tea".to_owned())],
    )]);
    let value = input.clone().into_value();
    assert_eq!(value.kind(), ValueKind::Map);
    assert_eq!(
        BTreeMap::<String, Vec<Option<String>>>::try_from_value(&value).unwrap(),
        input
    );
    assert!(().into_value().is_nil());
    assert_eq!(Option::<String>::try_from_value(&Value::Nil).unwrap(), None);
    assert_eq!(
        Option::<String>::try_from_value(&Value::from("coffee")).unwrap(),
        Some("coffee".to_owned())
    );

    let integer = Integer::from(9007199254740993_i64);
    let decimal = Decimal::from(123_i64);
    assert_eq!(
        Integer::try_from_value(&integer.clone().into_value()).unwrap(),
        integer
    );
    assert_eq!(
        Decimal::try_from_value(&decimal.clone().into_value()).unwrap(),
        decimal
    );
    assert_eq!(f64::try_from_value(&Value::from(1_f64)).unwrap(), 1.);

    for value in [
        Value::integer(1_i64),
        Decimal::from(1_i64).into_value(),
        Value::from("1"),
    ] {
        let error = f64::try_from_value(&value).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Runtime);
        assert!(error.message().starts_with("expected number, got "));
    }
    assert_eq!(
        Integer::try_from_value(&Value::from(1_f64))
            .unwrap_err()
            .message(),
        "expected integer, got number"
    );
    assert_eq!(
        Decimal::try_from_value(&Value::integer(1_i64))
            .unwrap_err()
            .message(),
        "expected decimal, got integer"
    );

    let nested = Value::map([(
        "settings",
        Value::array(vec![Value::from(true), Value::from("no")]),
    )]);
    let error = BTreeMap::<String, Vec<bool>>::try_from_value(&nested).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Runtime);
    assert_eq!(
        error.message(),
        "value at .settings: value at [1]: expected bool, got string"
    );
    assert!(Vec::<bool>::try_from_value(&Value::from("not an array")).is_err());
}

#[test]
fn checked_compilation_collects_ordered_named_parse_errors() {
    let engine = Engine::new();
    let errors = engine
        .check_program_named("virtual://rules.coffee", "first = [1 2]\nsecond = [3 4]\n")
        .expect_err("two independent malformed statements should be reported");
    assert_eq!(errors.len(), 2);
    assert_eq!(
        errors
            .iter()
            .map(|error| error.position().map(|position| position.line))
            .collect::<Vec<_>>(),
        vec![Some(1), Some(2)]
    );
    assert!(errors.iter().all(|error| {
        error.kind() == ErrorKind::Parse
            && error.labels()[0].span.source_name.as_deref() == Some("virtual://rules.coffee")
    }));

    let first_error = engine
        .compile_program_named("virtual://rules.coffee", "first = [1 2]\nsecond = [3 4]\n")
        .expect_err("the existing compile API remains first-error only");
    assert_eq!(
        first_error.position().map(|position| position.line),
        Some(1)
    );
}

#[test]
fn named_litcoffee_sources_execute_and_preserve_physical_diagnostic_lines() {
    let source = "# Rules\n\n    answer = 40\n    answer + 2\n";
    assert_eq!(
        Context::new()
            .eval_named("virtual://rules.litcoffee", source)
            .unwrap()
            .as_number(),
        Some(42.0)
    );
    Engine::new()
        .check_program_named("virtual://rules.litcoffee", source)
        .unwrap();

    let error = Context::new()
        .eval_named(
            "virtual://broken.litcoffee",
            "# Broken rule\n\n    missing + 1\n",
        )
        .unwrap_err();
    let position = error.position().expect("runtime error has a position");
    assert_eq!(position.line, 3);
    assert_eq!(position.column, None);
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("virtual://broken.litcoffee")
    );
}

#[test]
fn litcoffee_rejects_mixed_code_margins_with_a_named_physical_line() {
    let error = Engine::new()
        .compile_program_named("virtual://mixed.litcoffee", "    one = 1\n\n\ttwo = 2\n")
        .unwrap_err();
    assert_eq!(error.message(), "inconsistent literate code indentation");
    assert_eq!(error.position().map(|position| position.line), Some(3));
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("virtual://mixed.litcoffee")
    );
}

#[test]
fn checked_compilation_recovers_past_a_failed_indentation_block() {
    let errors = Engine::new()
        .check_program("if true\n  first = [1 2]\nsecond = [3 4]\n")
        .expect_err("dedent recovery should reach the following top-level statement");
    assert_eq!(
        errors
            .iter()
            .map(|error| error.position().map(|position| position.line))
            .collect::<Vec<_>>(),
        vec![Some(2), Some(3)]
    );
}

#[test]
fn checked_compilation_does_not_split_failed_control_flow_continuations() {
    let errors = Engine::new()
        .check_program("if true\n  first = [1 2]\nelse\n  fallback = 1\nsecond = [3 4]\n")
        .expect_err("the else clause belongs to the failed if statement");
    assert_eq!(
        errors
            .iter()
            .map(|error| error.position().map(|position| position.line))
            .collect::<Vec<_>>(),
        vec![Some(2), Some(5)]
    );

    let errors = Engine::new()
        .check_program(
            "try\n  first = [1 2]\ncatch problem\n  fallback = 1\nfinally\n  cleanup = 1\nsecond = [3 4]\n",
        )
        .expect_err("catch and finally clauses belong to the failed try statement");
    assert_eq!(
        errors
            .iter()
            .map(|error| error.position().map(|position| position.line))
            .collect::<Vec<_>>(),
        vec![Some(2), Some(7)]
    );
}

#[test]
fn checked_compilation_handles_many_independent_errors_in_one_pass() {
    use std::fmt::Write as _;

    let mut source = String::with_capacity(24 * 1_024);
    for index in 0..1_024 {
        writeln!(&mut source, "broken{index} = [1 2]").unwrap();
    }
    let errors = Engine::new()
        .check_program(&source)
        .expect_err("every malformed top-level statement should be reported");
    assert_eq!(errors.len(), 1_024);
    assert_eq!(errors[0].position().map(|position| position.line), Some(1));
    assert_eq!(
        errors
            .last()
            .and_then(|error| error.position())
            .map(|position| position.line),
        Some(1_024)
    );
}

#[test]
fn builder_embedding_surface_chains_host_configuration() {
    let program = Engine::new()
        .compile_program("host(20, 22) * factor")
        .unwrap();
    let mut context = Context::new()
        .with_global("factor", Value::from(2_f64))
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
fn contexts_keep_builtin_binding_replacements_isolated() {
    let mut replaced = Context::new();
    replaced.set_global("len", Value::from(42_f64));

    let mut fresh = Context::new();
    assert_eq!(replaced.eval("len").unwrap().as_number(), Some(42.));
    assert_eq!(fresh.eval("len([20, 22])").unwrap().as_number(), Some(2.));
}

#[test]
fn lazily_promoted_builtins_remain_shadowable_across_shared_programs() {
    let program = Engine::new().compile_program("len([20, 22])").unwrap();
    let mut replaced = Context::new();
    assert_eq!(
        replaced.run_program(&program).unwrap().as_number(),
        Some(2.)
    );
    replaced.add_native("len", |_| Ok(Value::from(7_f64)));
    assert_eq!(
        replaced.run_program(&program).unwrap().as_number(),
        Some(7.)
    );

    let mut fresh = Context::new();
    assert_eq!(fresh.run_program(&program).unwrap().as_number(), Some(2.));
}

#[test]
fn public_values_and_native_errors_are_structured() {
    let mut context = Context::new();
    context.set_global(
        "host_values",
        Value::map([
            ("answer", Value::from(42_f64)),
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
fn classes_cross_the_embedding_boundary_as_opaque_values_with_named_diagnostics() {
    let mut context = Context::new();
    let values = context
        .eval("class Point\n  constructor: (@x) ->\n  value: -> @x\np = new Point(42)\n[Point, p, p.value()]")
        .unwrap();
    let values = values.as_array().unwrap();
    assert_eq!(values[0].kind(), ValueKind::Class);
    assert_eq!(values[1].kind(), ValueKind::Instance);
    assert_eq!(values[2].as_number(), Some(42.));
    assert!(values[0].as_map().is_none());
    assert!(values[1].as_map().is_none());
    let stats = context.last_execution();
    assert!(stats.calls >= 2);
    assert!(stats.container_ops >= 4);
    assert!(stats.value_allocations >= 4);
    assert!(stats.environment_allocations >= 2);

    let program = Engine::new()
        .compile_program_named(
            "virtual://broken-class.coffee",
            "class Broken\n  fail: -> missing\nnew Broken().fail()",
        )
        .unwrap();
    let error = Context::new().run_program(&program).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Runtime);
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("virtual://broken-class.coffee")
    );
    assert_eq!(error.labels()[0].span.start.line, 2);
    assert!(error.labels().iter().any(|label| {
        label.kind == DiagnosticLabelKind::Secondary
            && label.span.source_name.as_deref() == Some("virtual://broken-class.coffee")
            && label.span.start.line == 3
    }));
}

#[test]
fn host_domain_errors_cross_catch_and_embedding_boundaries() {
    let mut context = Context::new();
    context.add_native("charge", |_| {
        Err(Error::domain(
            "payment.declined",
            "card declined",
            Value::map([("retryable", Value::from(false))]),
        ))
    });
    let caught = context
        .eval("try charge() catch problem then [problem.code, problem.data.retryable]")
        .unwrap();
    assert_eq!(caught.to_string(), "[payment.declined, false]");

    let error = context.eval("charge()").unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Runtime);
    assert_eq!(error.message(), "card declined");
    let script = error.script_error().unwrap();
    assert_eq!(script.code(), "payment.declined");
    assert_eq!(
        script.data().as_map().unwrap()["retryable"].as_bool(),
        Some(false)
    );
    assert_eq!(
        error.to_string(),
        "runtime error [payment.declined]: card declined"
    );
}

#[test]
fn source_diagnostics_expose_primary_labels_and_precise_columns() {
    let error = Engine::new().compile("value = 1\n@").unwrap_err();
    let position = error.position().expect("parse error has a position");
    assert_eq!(position.line, 2);
    assert_eq!(position.column, Some(1));

    let [label] = error.labels() else {
        panic!("parse error must have one primary label");
    };
    assert_eq!(label.kind, DiagnosticLabelKind::Primary);
    assert_eq!(label.span.source_name, None);
    assert_eq!(label.span.start, position);
    assert_eq!(
        label.span.end,
        Some(quickcoffee::SourcePosition {
            line: 2,
            column: Some(2),
        })
    );
    assert_eq!(label.message, None);

    let runtime = Error::runtime("host failed");
    assert!(runtime.position().is_none());
    assert!(runtime.labels().is_empty());
}

#[test]
fn named_embedding_sources_preserve_opaque_names_without_changing_anonymous_calls() {
    const NAME: &str = "virtual://rules/../invoice.coffee";
    let engine = Engine::new();
    let error = engine.compile_named(NAME, "value = 1\n@").unwrap_err();
    assert_eq!(error.labels()[0].span.source_name.as_deref(), Some(NAME));

    let error = engine.compile_program_named(NAME, "@").unwrap_err();
    assert_eq!(error.labels()[0].span.source_name.as_deref(), Some(NAME));
    let error = Context::new().eval_named(NAME, "@").unwrap_err();
    assert_eq!(error.labels()[0].span.source_name.as_deref(), Some(NAME));

    let error = quickcoffee::compile_named(NAME, "@").unwrap_err();
    assert_eq!(error.labels()[0].span.source_name.as_deref(), Some(NAME));
    let error = quickcoffee::compile_program_named(NAME, "@").unwrap_err();
    assert_eq!(error.labels()[0].span.source_name.as_deref(), Some(NAME));
    let error = quickcoffee::eval_named(NAME, "@").unwrap_err();
    assert_eq!(error.labels()[0].span.source_name.as_deref(), Some(NAME));

    let anonymous = engine.compile("@").unwrap_err();
    assert_eq!(anonymous.labels()[0].span.source_name, None);
}

#[test]
fn compiled_program_source_maps_attribute_runtime_and_resource_errors() {
    const NAME: &str = "virtual://runtime/invoice.coffee";
    let mut context = Context::new();

    let top_level = context
        .eval_named(NAME, "value = 1\nvalue + 'x'")
        .unwrap_err();
    assert_eq!(top_level.kind(), ErrorKind::Runtime);
    let span = &top_level.labels()[0].span;
    assert_eq!(span.source_name.as_deref(), Some(NAME));
    assert_eq!(span.start.line, 2);
    assert_eq!(span.start.column, Some(1));
    assert_eq!(
        span.end,
        Some(quickcoffee::SourcePosition {
            line: 2,
            column: Some(6),
        })
    );
    assert_eq!(
        top_level.to_string(),
        "runtime error: expected matching number or integer operands"
    );

    let unicode = context
        .eval_named(NAME, "状态 = 1\n状态 + 'x'")
        .unwrap_err();
    let span = &unicode.labels()[0].span;
    assert_eq!(span.start.line, 2);
    assert_eq!(span.start.column, Some(1));
    assert_eq!(
        span.end,
        Some(quickcoffee::SourcePosition {
            line: 2,
            column: Some(3),
        })
    );

    let rewritten = context
        .eval_named(NAME, "record =\n  key: 1 + 'x'")
        .unwrap_err();
    let span = &rewritten.labels()[0].span;
    assert_eq!(span.start.line, 2);
    assert_eq!(span.start.column, None);
    assert_eq!(span.end, None);

    let function = context
        .eval_named(NAME, "fail = (value) ->\n  value + 'x'\nfail(1)")
        .unwrap_err();
    let span = &function.labels()[0].span;
    assert_eq!(span.source_name.as_deref(), Some(NAME));
    assert_eq!(span.start.line, 2);
    assert_eq!(span.start.column, Some(3));
    assert_eq!(
        span.end,
        Some(quickcoffee::SourcePosition {
            line: 2,
            column: Some(8),
        })
    );

    let default = context
        .eval_named(NAME, "f = (value = missing) -> value\nf()")
        .unwrap_err();
    let span = &default.labels()[0].span;
    assert_eq!(span.source_name.as_deref(), Some(NAME));
    assert_eq!(span.start.line, 1);
    assert_eq!(span.start.column, Some(14));
    assert_eq!(
        span.end,
        Some(quickcoffee::SourcePosition {
            line: 1,
            column: Some(21),
        })
    );

    let destructure = context.eval_named(NAME, "[left, right] = [1]").unwrap_err();
    let span = &destructure.labels()[0].span;
    assert_eq!(span.source_name.as_deref(), Some(NAME));
    assert_eq!(span.start.column, Some(1));
    assert_eq!(span.end.unwrap().column, Some(2));

    context.add_native("host_fail", |_| Err(Error::runtime("host failed")));
    let native = context.eval_named(NAME, "host_fail()").unwrap_err();
    let span = &native.labels()[0].span;
    assert_eq!(span.source_name.as_deref(), Some(NAME));
    assert_eq!(span.start.column, Some(1));
    assert_eq!(span.end.unwrap().column, Some(10));

    context.set_fuel(4);
    let resource = context.eval_named(NAME, "while true then 1").unwrap_err();
    assert_eq!(resource.kind(), ErrorKind::Resource);
    assert_eq!(resource.labels()[0].span.source_name.as_deref(), Some(NAME));
    assert_eq!(resource.labels()[0].span.start.line, 1);

    let anonymous = Context::new().eval("missing").unwrap_err();
    assert_eq!(anonymous.labels()[0].span.source_name, None);
    assert_eq!(anonymous.labels()[0].span.start.column, Some(1));

    let mut retained = Context::new();
    retained
        .eval_named(
            "virtual://definitions.coffee",
            "fail = (value) -> value + 'x'",
        )
        .unwrap();
    let retained_error = retained
        .eval_named("virtual://caller.coffee", "fail(1)")
        .unwrap_err();
    assert_eq!(
        retained_error.labels()[0].span.source_name.as_deref(),
        Some("virtual://definitions.coffee")
    );
}

#[test]
fn runtime_errors_keep_ordered_secondary_quickcoffee_call_sites() {
    const NAME: &str = "virtual://runtime/call-stack.coffee";
    let error = Context::new()
        .eval_named(NAME, "outer = -> inner()\ninner = -> missing\nouter()")
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Runtime);
    assert_eq!(error.to_string(), "runtime error: unknown name 'missing'");
    assert_eq!(error.labels().len(), 3);

    let primary = &error.labels()[0];
    assert_eq!(primary.kind, quickcoffee::DiagnosticLabelKind::Primary);
    assert_eq!(primary.span.source_name.as_deref(), Some(NAME));
    assert_eq!(primary.span.start.line, 2);
    assert_eq!(primary.span.start.column, Some(12));

    let inner_call = &error.labels()[1];
    assert_eq!(inner_call.kind, quickcoffee::DiagnosticLabelKind::Secondary);
    assert_eq!(inner_call.message.as_deref(), Some("called from here"));
    assert_eq!(inner_call.span.source_name.as_deref(), Some(NAME));
    assert_eq!(inner_call.span.start.line, 1);
    assert_eq!(inner_call.span.start.column, Some(12));

    let outer_call = &error.labels()[2];
    assert_eq!(outer_call.kind, quickcoffee::DiagnosticLabelKind::Secondary);
    assert_eq!(outer_call.message.as_deref(), Some("called from here"));
    assert_eq!(outer_call.span.source_name.as_deref(), Some(NAME));
    assert_eq!(outer_call.span.start.line, 3);
    assert_eq!(outer_call.span.start.column, Some(1));

    let mut retained = Context::new();
    retained
        .eval_named("virtual://definitions.coffee", "fail = -> missing")
        .unwrap();
    let cross_eval = retained
        .eval_named("virtual://caller.coffee", "fail()")
        .unwrap_err();
    assert_eq!(cross_eval.labels().len(), 2);
    assert_eq!(
        cross_eval.labels()[0].span.source_name.as_deref(),
        Some("virtual://definitions.coffee")
    );
    assert_eq!(
        cross_eval.labels()[1].span.source_name.as_deref(),
        Some("virtual://caller.coffee")
    );
    assert_eq!(
        cross_eval.labels()[1].kind,
        quickcoffee::DiagnosticLabelKind::Secondary
    );

    let resource = Context::new()
        .with_fuel(12)
        .eval_named(NAME, "spin = -> while true then 1\nspin()")
        .unwrap_err();
    assert_eq!(resource.kind(), ErrorKind::Resource);
    assert!(resource.labels().len() >= 2);
    assert_eq!(resource.labels()[0].span.source_name.as_deref(), Some(NAME));
    assert_eq!(
        resource.labels()[1].kind,
        quickcoffee::DiagnosticLabelKind::Secondary
    );
    assert_eq!(resource.labels()[1].span.source_name.as_deref(), Some(NAME));
    assert_eq!(resource.labels()[1].span.start.line, 2);

    assert_eq!(
        Context::new()
            .eval("try missing catch error then 42")
            .unwrap()
            .as_number(),
        Some(42.)
    );
}

#[test]
fn raw_chunks_do_not_invent_source_attribution() {
    let error = Context::new()
        .run(quickcoffee::Chunk {
            constants: vec![],
            code: vec![
                quickcoffee::Instruction::Load("missing".to_owned()),
                quickcoffee::Instruction::Return,
            ],
        })
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Runtime);
    assert!(error.labels().is_empty());
}

#[test]
fn program_source_maps_do_not_change_bytecode_views() {
    let source = "square = (value) -> value * value\nsquare(7)";
    let chunk = Engine::new().compile(source).unwrap();
    let program = Engine::new()
        .compile_program_named("virtual://square.coffee", source)
        .unwrap();
    assert_eq!(program.fingerprint(), chunk.fingerprint());
    assert_eq!(program.disassemble(), chunk.disassemble());
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
    assert_eq!(stats.managed_objects_allocated, 3);
    assert_eq!(stats.managed_bytes_allocated, 16);

    context.add_native("host_array", |_| Ok(Value::array(vec![Value::from(1_f64)])));
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
    assert_eq!(stats.managed_objects_allocated, 0);
    assert_eq!(stats.managed_bytes_allocated, 0);

    assert_eq!(
        context
            .eval("keys({a: 1, b: 2})")
            .unwrap()
            .as_array()
            .map(<[Value]>::len),
        Some(2)
    );
    let stats = context.last_execution();
    assert_eq!(stats.value_allocations, 3);
    assert_eq!(stats.managed_objects_allocated, 3);
    assert_eq!(stats.managed_bytes_allocated, 18);
}

#[test]
fn managed_allocation_telemetry_is_deterministic_across_value_kinds_and_runs() {
    let mut context = Context::new();

    assert_eq!(
        context
            .eval("integer(256) + integer(1)")
            .unwrap()
            .to_string(),
        "257n"
    );
    let exact = context.last_execution();
    assert_eq!(exact.value_allocations, 2);
    assert_eq!(exact.managed_objects_allocated, 3);
    assert_eq!(exact.managed_bytes_allocated, 5);

    assert_eq!(
        context
            .eval("try throw 'boom' catch error then error.code")
            .unwrap()
            .to_string(),
        "throw"
    );
    let caught = context.last_execution();
    assert_eq!(caught.managed_objects_allocated, 2);
    assert_eq!(caught.managed_bytes_allocated, 66);

    let class_source = "class Point\n  constructor: (@x) ->\nnew Point(1)";
    assert_eq!(
        context.eval(class_source).unwrap().to_string(),
        "<Point instance>"
    );
    let class = context.last_execution();
    assert_eq!(class.managed_objects_allocated, 4);
    assert_eq!(class.managed_bytes_allocated, 54);

    let defaults = "outer = (value = ['x']) -> value\nouter()";
    assert_eq!(context.eval(defaults).unwrap().to_string(), "[x]");
    let first = context.last_execution();
    assert_eq!(first.managed_objects_allocated, 2);
    assert_eq!(first.managed_bytes_allocated, 8);
    assert_eq!(context.eval(defaults).unwrap().to_string(), "[x]");
    assert_eq!(context.last_execution(), first);
}

#[test]
fn managed_allocation_telemetry_survives_errors_and_pattern_rollback() {
    let mut context = Context::new();
    assert!(context.eval("created = [len('x')]\nmissing").is_err());
    let failed = context.last_execution();
    assert_eq!(failed.managed_objects_allocated, 1);
    assert_eq!(failed.managed_bytes_allocated, 8);

    let rollback =
        "try\n  [head, tail...] = [1]\n  [required, missing] = [1]\ncatch error\n  error.code";
    assert_eq!(context.eval(rollback).unwrap().to_string(), "runtime");
    let first = context.last_execution();
    assert!(first.managed_objects_allocated > 0);
    assert!(first.managed_bytes_allocated > 0);
    assert_eq!(context.eval(rollback).unwrap().to_string(), "runtime");
    assert_eq!(context.last_execution(), first);
}

#[test]
fn transient_managed_allocation_limits_are_per_run_atomic_and_uncatchable() {
    let source = "make = -> ['temporary']\nmake()\nmake()\n42";
    let mut baseline = Context::new();
    assert_eq!(baseline.eval(source).unwrap().as_number(), Some(42.));
    let expected = baseline.last_execution();
    assert!(expected.managed_objects_allocated > 1);
    assert!(expected.managed_bytes_allocated > 1);

    let exact_limits = ResourceLimits::default()
        .with_max_transient_managed_objects(expected.managed_objects_allocated)
        .with_max_transient_managed_bytes(expected.managed_bytes_allocated);
    let mut exact = Context::new().with_resource_limits(exact_limits);
    assert_eq!(exact.eval(source).unwrap().as_number(), Some(42.));
    assert_eq!(exact.last_execution(), expected);
    assert_eq!(exact.eval(source).unwrap().as_number(), Some(42.));
    assert_eq!(exact.last_execution(), expected);

    let mut object_limited = Context::new().with_resource_limits(
        ResourceLimits::default()
            .with_max_transient_managed_objects(expected.managed_objects_allocated - 1),
    );
    let error = object_limited
        .eval_named(
            "transient-objects.coffee",
            "try\n  make = -> ['temporary']\n  make()\n  make()\n  42\ncatch ignored\n  99",
        )
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Resource);
    assert_eq!(
        error.resource_limit(),
        Some(ResourceLimit::TransientManagedObjects)
    );
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("transient-objects.coffee")
    );
    assert!(
        object_limited.last_execution().managed_objects_allocated
            > expected.managed_objects_allocated - 1
    );
    assert!(object_limited.get_global("make").is_none());

    let mut byte_limited = Context::new().with_resource_limits(
        ResourceLimits::default()
            .with_max_transient_managed_bytes(expected.managed_bytes_allocated - 1),
    );
    let error = byte_limited.eval(source).unwrap_err();
    assert_eq!(
        error.resource_limit(),
        Some(ResourceLimit::TransientManagedBytes)
    );
    assert!(
        byte_limited.last_execution().managed_bytes_allocated
            > expected.managed_bytes_allocated - 1
    );
    assert!(byte_limited.get_global("make").is_none());

    let defaults = ResourceLimits::default();
    assert_eq!(defaults.max_transient_managed_objects(), u64::MAX);
    assert_eq!(defaults.max_transient_managed_bytes(), u64::MAX);
}

#[test]
fn contextual_native_allocation_limits_override_callback_errors_and_keep_stats() {
    let mut objects = Context::new()
        .with_resource_limits(ResourceLimits::default().with_max_transient_managed_objects(1))
        .with_contextual_native("host_work", |call, _| {
            call.record_managed_allocation(2, 0);
            Err(Error::runtime("host failure after allocation"))
        });
    let error = objects
        .eval("try host_work() catch ignored then 42")
        .unwrap_err();
    assert_eq!(
        error.resource_limit(),
        Some(ResourceLimit::TransientManagedObjects)
    );
    assert_eq!(objects.last_execution().managed_objects_allocated, 2);

    let mut bytes = Context::new()
        .with_resource_limits(ResourceLimits::default().with_max_transient_managed_bytes(8))
        .with_contextual_native("host_work", |call, _| {
            call.record_managed_allocation(0, 9);
            Ok(Value::Nil)
        });
    let error = bytes.eval("host_work()").unwrap_err();
    assert_eq!(
        error.resource_limit(),
        Some(ResourceLimit::TransientManagedBytes)
    );
    assert_eq!(bytes.last_execution().managed_bytes_allocated, 9);
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
fn json_resource_policy_is_replaceable_labeled_and_uncatchable() {
    let defaults = ResourceLimits::default();
    let constrained = defaults
        .with_max_json_input_bytes(32)
        .with_max_json_output_bytes(32)
        .with_max_json_string_bytes(16)
        .with_max_json_container_items(4)
        .with_max_json_values(1)
        .with_max_json_nesting_depth(2);
    let mut context = Context::new().with_resource_limits(constrained);
    assert_eq!(context.resource_limits(), constrained);
    assert_eq!(context.resource_limits().max_json_input_bytes(), 32);
    assert_eq!(context.resource_limits().max_json_output_bytes(), 32);
    assert_eq!(context.resource_limits().max_json_string_bytes(), 16);
    assert_eq!(context.resource_limits().max_json_container_items(), 4);
    assert_eq!(context.resource_limits().max_json_values(), 1);
    assert_eq!(context.resource_limits().max_json_nesting_depth(), 2);

    let error = context
        .eval_named(
            "virtual://limits.coffee",
            "try parse_json('[0]') catch ignored then 42",
        )
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Resource);
    assert_eq!(error.resource_limit(), Some(ResourceLimit::JsonValueCount));
    assert!(error.message().contains("value count exceeds 1"));
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("virtual://limits.coffee")
    );
    assert_eq!(error.labels()[0].span.start.line, 1);

    context.set_resource_limits(defaults);
    assert_eq!(context.resource_limits(), defaults);
    assert_eq!(
        context
            .eval("parse_json('[0]')[0]")
            .unwrap()
            .as_integer()
            .and_then(|value| value.as_i64()),
        Some(0)
    );

    let caught = context
        .eval("try parse_json('[0,]') catch problem then problem.code")
        .unwrap();
    assert_eq!(caught.as_str(), Some("json.parse"));
}

#[test]
fn collection_operation_resource_policy_is_replaceable_labeled_and_uncatchable() {
    let defaults = ResourceLimits::default();
    let constrained = defaults.with_max_collection_operation_items(2);
    assert_eq!(constrained.max_collection_operation_items(), 2);

    let mut context = Context::new().with_resource_limits(constrained);
    context.eval("items = [3, 2, 1]").unwrap();
    let error = context
        .eval_named(
            "virtual://collection-limits.coffee",
            "try sort(items) catch ignored then [0]",
        )
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Resource);
    assert_eq!(
        error.resource_limit(),
        Some(ResourceLimit::CollectionOperationItems)
    );
    assert!(error.message().contains("sort input exceeds 2 items"));
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("virtual://collection-limits.coffee")
    );
    assert_eq!(context.eval("items").unwrap().to_string(), "[3, 2, 1]");

    context.set_resource_limits(defaults);
    assert_eq!(
        context.eval("sort(items)").unwrap().to_string(),
        "[1, 2, 3]"
    );
}

#[test]
fn concat_checks_output_and_operation_limits_before_copying() {
    let defaults = ResourceLimits::default();
    let mut context = Context::new();
    context
        .eval("left = [1, 2]\nright = [3, 4]\nprefix = 'abc'\nsuffix = 'def'")
        .unwrap();

    context.set_resource_limits(defaults.with_max_string_bytes(5).with_max_array_items(3));
    let error = context
        .eval_named(
            "virtual://concat-limits.coffee",
            "try concat(prefix, suffix) catch ignored then 'caught'",
        )
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Resource);
    assert_eq!(error.resource_limit(), Some(ResourceLimit::StringBytes));
    assert!(error.message().contains("string exceeds 5 bytes"));
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("virtual://concat-limits.coffee")
    );

    let error = context
        .eval("try concat(left, right) catch ignored then []")
        .unwrap_err();
    assert_eq!(error.resource_limit(), Some(ResourceLimit::ArrayItems));
    assert_eq!(context.eval("left").unwrap().to_string(), "[1, 2]");
    assert_eq!(context.eval("right").unwrap().to_string(), "[3, 4]");

    context.set_resource_limits(
        defaults
            .with_max_array_items(4)
            .with_max_collection_operation_items(3),
    );
    let error = context.eval("concat(left, right)").unwrap_err();
    assert_eq!(
        error.resource_limit(),
        Some(ResourceLimit::CollectionOperationItems)
    );
    assert!(error.message().contains("concat input exceeds 3 items"));

    context.set_resource_limits(defaults);
    assert_eq!(
        context.eval("concat(left, right)").unwrap().to_string(),
        "[1, 2, 3, 4]"
    );
}

#[test]
fn literal_replacement_checks_input_and_output_before_allocation() {
    let defaults = ResourceLimits::default();
    let constrained = defaults.with_max_text_operation_bytes(5);
    assert_eq!(constrained.max_text_operation_bytes(), 5);

    let mut context = Context::new();
    context.eval("text = 'banana'").unwrap();
    context.set_resource_limits(constrained);
    let error = context
        .eval_named(
            "virtual://replace-limits.coffee",
            "try replace_all(text, 'a', 'x') catch ignored then 'caught'",
        )
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Resource);
    assert_eq!(
        error.resource_limit(),
        Some(ResourceLimit::TextOperationBytes)
    );
    assert!(
        error
            .message()
            .contains("replace_all input exceeds 5 UTF-8 bytes")
    );
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("virtual://replace-limits.coffee")
    );
    assert_eq!(context.eval("text").unwrap().as_str(), Some("banana"));

    context.set_resource_limits(
        defaults
            .with_max_text_operation_bytes(6)
            .with_max_string_bytes(7),
    );
    let error = context.eval("replace_all(text, 'a', 'xxx')").unwrap_err();
    assert_eq!(error.resource_limit(), Some(ResourceLimit::StringBytes));
    assert_eq!(context.eval("text").unwrap().as_str(), Some("banana"));

    context.set_resource_limits(defaults);
    assert_eq!(
        context
            .eval("replace_all(text, 'a', 'x')")
            .unwrap()
            .as_str(),
        Some("bxnxnx")
    );
}

#[test]
fn general_value_resource_policy_is_replaceable_atomic_and_uncatchable() {
    let defaults = ResourceLimits::default();
    let constrained = defaults
        .with_max_string_bytes(3)
        .with_max_array_items(2)
        .with_max_map_entries(1);
    assert_eq!(constrained.max_string_bytes(), 3);
    assert_eq!(constrained.max_array_items(), 2);
    assert_eq!(constrained.max_map_entries(), 1);

    let mut context = Context::new().with_resource_limits(constrained);
    context.eval("items = [1, 2]").unwrap();
    let error = context
        .eval_named(
            "virtual://value-limits.coffee",
            "try items = [1, 2, 3] catch ignored then [0]",
        )
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Resource);
    assert_eq!(error.resource_limit(), Some(ResourceLimit::ArrayItems));
    assert!(error.message().contains("array exceeds 2 items"));
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("virtual://value-limits.coffee")
    );

    context.set_resource_limits(defaults);
    assert_eq!(context.eval("items").unwrap().to_string(), "[1, 2]");

    context.set_resource_limits(constrained);
    let error = context
        .eval("try 'four' catch ignored then 'ok'")
        .unwrap_err();
    assert_eq!(error.resource_limit(), Some(ResourceLimit::StringBytes));
    let error = context
        .eval("try {a: 1, b: 2} catch ignored then nil")
        .unwrap_err();
    assert_eq!(error.resource_limit(), Some(ResourceLimit::MapEntries));
    let error = context
        .eval("try join([1, 2], '--') catch ignored then nil")
        .unwrap_err();
    assert_eq!(error.resource_limit(), Some(ResourceLimit::StringBytes));

    context.set_resource_limits(defaults.with_max_string_bytes(5).with_max_array_items(2));
    let error = context
        .eval("try split('a,b,c', ',') catch ignored then nil")
        .unwrap_err();
    assert_eq!(error.resource_limit(), Some(ResourceLimit::ArrayItems));

    let mut host_value = Context::new()
        .with_resource_limits(constrained)
        .with_global(
            "host_items",
            Value::array(vec![
                Value::from(1_i64),
                Value::from(2_i64),
                Value::from(3_i64),
            ]),
        );
    let error = host_value.eval("host_items").unwrap_err();
    assert_eq!(error.resource_limit(), Some(ResourceLimit::ArrayItems));

    let mut nested_host_value = Context::new().with_global(
        "nested_host_value",
        Value::map([
            ("scalar", Value::from(1_i64)),
            (
                "nested",
                Value::array(vec![Value::map([("payload", Value::from("oversized"))])]),
            ),
        ]),
    );
    nested_host_value.set_resource_limits(ResourceLimits::default().with_max_string_bytes(3));
    let error = nested_host_value.eval("nested_host_value").unwrap_err();
    assert_eq!(error.resource_limit(), Some(ResourceLimit::StringBytes));
    assert!(error.message().contains("string exceeds 3 bytes"));
}

#[test]
fn member_reads_recheck_non_scalar_values_against_current_resource_limits() {
    let mut context = Context::new();
    context
        .eval("class Entry\n  constructor: (@text) ->\nentry = new Entry('oversized')")
        .unwrap();
    context.set_resource_limits(ResourceLimits::default().with_max_string_bytes(3));

    let error = context
        .eval("try entry.text catch ignored then 'caught'")
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Resource);
    assert_eq!(error.resource_limit(), Some(ResourceLimit::StringBytes));
    assert!(error.message().contains("string exceeds 3 bytes"));
}

#[test]
fn exact_numeric_resource_policy_covers_constants_globals_operations_and_json() {
    let defaults = ResourceLimits::default();
    let constrained = defaults
        .with_max_integer_bits(3)
        .with_max_decimal_coefficient_bits(4)
        .with_max_decimal_scale(2);
    assert_eq!(constrained.max_integer_bits(), 3);
    assert_eq!(constrained.max_decimal_coefficient_bits(), 4);
    assert_eq!(constrained.max_decimal_scale(), 2);

    let engine = Engine::new();
    let program = engine
        .compile_program("try 8n catch ignored then 1n")
        .unwrap();
    let fingerprint = program.fingerprint();

    let mut constrained_context = Context::new().with_resource_limits(constrained);
    let error = constrained_context
        .run_program(&program)
        .expect_err("numeric resource failures must bypass script catch handlers");
    assert_eq!(error.kind(), ErrorKind::Resource);
    assert_eq!(error.resource_limit(), Some(ResourceLimit::IntegerBits));
    assert_eq!(program.fingerprint(), fingerprint);

    let mut default_context = Context::new();
    assert_eq!(
        default_context
            .run_program(&program)
            .unwrap()
            .as_integer()
            .and_then(|value| value.as_i64()),
        Some(8)
    );
    assert_eq!(program.fingerprint(), fingerprint);

    assert!(constrained_context.eval("7n").is_ok());
    assert!(constrained_context.eval("9m").is_ok());
    assert!(constrained_context.eval("0.01m").is_ok());
    assert_eq!(
        constrained_context
            .eval("value = 1n\namount = 4n\nvalue >> amount")
            .unwrap()
            .as_integer()
            .and_then(|value| value.as_i64()),
        Some(0)
    );

    let zero_limits = defaults
        .with_max_integer_bits(0)
        .with_max_decimal_coefficient_bits(0)
        .with_max_decimal_scale(0);
    let mut zero_context = Context::new().with_resource_limits(zero_limits);
    for source in ["0n", "0m", "decimal('0e2')", "parse_json('0e2')"] {
        assert!(zero_context.eval(source).is_ok(), "{source}");
    }

    for (source, limit) in [
        ("x = 7n\nx + 1n", ResourceLimit::IntegerBits),
        ("x = 7n\n++x", ResourceLimit::IntegerBits),
        ("x = 3n\nx * 3n", ResourceLimit::IntegerBits),
        ("x = 1n\nx << 3n", ResourceLimit::IntegerBits),
        ("x = 2n\nx ** 3n", ResourceLimit::IntegerBits),
        ("sum([4n, 4n])", ResourceLimit::IntegerBits),
        ("16m", ResourceLimit::DecimalCoefficientBits),
        ("0.001m", ResourceLimit::DecimalScale),
        ("x = 1m\nx + 0.01m", ResourceLimit::DecimalCoefficientBits),
        ("x = 4m\nx * 4m", ResourceLimit::DecimalCoefficientBits),
        ("x = 4m\nx ** 2m", ResourceLimit::DecimalCoefficientBits),
        ("x = 1m\nx / 8m", ResourceLimit::DecimalScale),
        (
            "decimal_div(1m, 2m, 3, 'half_even')",
            ResourceLimit::DecimalScale,
        ),
        (
            "round_decimal(1m, 3, 'half_even')",
            ResourceLimit::DecimalScale,
        ),
        ("integer(8)", ResourceLimit::IntegerBits),
        ("decimal('16')", ResourceLimit::DecimalCoefficientBits),
        ("parse_json('8')", ResourceLimit::IntegerBits),
        ("parse_json('16.0')", ResourceLimit::DecimalCoefficientBits),
        ("parse_json('0.001')", ResourceLimit::DecimalScale),
        ("parse_json('1e100001')", ResourceLimit::DecimalScale),
    ] {
        let error = constrained_context.eval(source).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Resource, "{source}");
        assert_eq!(error.resource_limit(), Some(limit), "{source}");
    }

    let alignment_limits = defaults
        .with_max_decimal_coefficient_bits(13)
        .with_max_decimal_scale(4);
    let mut alignment_context = Context::new().with_resource_limits(alignment_limits);
    for source in [
        "left = 0.9m\nright = 0.8001m\nleft < right",
        "min([0.9m, 0.8001m])",
        "sort([0.9m, 0.8001m])",
    ] {
        let error = alignment_context.eval(source).unwrap_err();
        assert_eq!(
            error.resource_limit(),
            Some(ResourceLimit::DecimalCoefficientBits),
            "{source}"
        );
    }

    constrained_context.set_global("host_integer", Value::from(8_i64));
    let error = constrained_context
        .eval_named("virtual://numeric-limits.coffee", "host_integer")
        .unwrap_err();
    assert_eq!(error.resource_limit(), Some(ResourceLimit::IntegerBits));
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("virtual://numeric-limits.coffee")
    );
    assert_eq!(
        constrained_context
            .get_global("host_integer")
            .unwrap()
            .as_integer()
            .and_then(|value| value.as_i64()),
        Some(8)
    );

    constrained_context.add_native("host_integer_result", |_| Ok(Value::from(8_i64)));
    let error = constrained_context
        .eval("host_integer_result()")
        .unwrap_err();
    assert_eq!(error.resource_limit(), Some(ResourceLimit::IntegerBits));

    constrained_context.set_global("stable", Value::from(7_i64));
    let error = constrained_context
        .eval("stable = stable + 1n")
        .unwrap_err();
    assert_eq!(error.resource_limit(), Some(ResourceLimit::IntegerBits));
    assert_eq!(
        constrained_context
            .get_global("stable")
            .unwrap()
            .as_integer()
            .and_then(|value| value.as_i64()),
        Some(7),
        "a failed numeric operation must not reach its following store"
    );

    let error = constrained_context
        .eval("fallback = (value = 8n) -> value\nfallback()")
        .unwrap_err();
    assert_eq!(error.resource_limit(), Some(ResourceLimit::IntegerBits));

    assert_eq!(
        constrained_context
            .eval("try parse_json('1e') catch problem then problem.code")
            .unwrap()
            .as_str(),
        Some("json.parse")
    );

    constrained_context.set_resource_limits(defaults);
    assert_eq!(
        constrained_context
            .eval("host_integer + host_integer_result()")
            .unwrap()
            .as_integer()
            .and_then(|value| value.as_i64()),
        Some(16)
    );
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

#[test]
fn signed_host_integers_cross_the_embedding_boundary_exactly() {
    let value = Value::from(i64::MAX);
    assert_eq!(value.kind(), quickcoffee::ValueKind::Integer);
    assert_eq!(value.as_integer().unwrap().as_i64(), Some(i64::MAX));
}

#[test]
fn exact_decimals_cross_the_embedding_boundary_losslessly() {
    let decimal = quickcoffee::Decimal::parse("-1234567890.012300").unwrap();
    assert_eq!(decimal.to_plain_string(), "-1234567890.0123");
    assert_eq!(decimal.scale(), 4);
    assert_eq!(decimal.coefficient().to_decimal_string(), "-12345678900123");

    let rebuilt = quickcoffee::Decimal::from_parts(decimal.coefficient(), decimal.scale()).unwrap();
    assert_eq!(rebuilt, decimal);
    let value = Value::from(rebuilt);
    assert_eq!(value.kind(), quickcoffee::ValueKind::Decimal);
    assert_eq!(
        value.as_decimal().unwrap().to_plain_string(),
        "-1234567890.0123"
    );
}
