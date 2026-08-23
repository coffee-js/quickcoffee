use quickcoffee::{
    CancellationToken, Context, DiagnosticLabelKind, Engine, Error, ErrorKind, ResourceLimit,
    Value, ValueKind,
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
fn checked_compilation_collects_ordered_named_parse_errors() {
    let engine = Engine::new();
    let errors = engine
        .check_program_named("virtual://rules.qc", "first = [1 2]\nsecond = [3 4]\n")
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
            && error.labels()[0].span.source_name.as_deref() == Some("virtual://rules.qc")
    }));

    let first_error = engine
        .compile_program_named("virtual://rules.qc", "first = [1 2]\nsecond = [3 4]\n")
        .expect_err("the existing compile API remains first-error only");
    assert_eq!(
        first_error.position().map(|position| position.line),
        Some(1)
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
    let source = (0..1_024)
        .map(|index| format!("broken{index} = [1 2]\n"))
        .collect::<String>();
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
    const NAME: &str = "virtual://rules/../invoice.qc";
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
    const NAME: &str = "virtual://runtime/invoice.qc";
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
    assert_eq!(top_level.to_string(), "runtime error: expected number");

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
        .eval_named("virtual://definitions.qc", "fail = (value) -> value + 'x'")
        .unwrap();
    let retained_error = retained
        .eval_named("virtual://caller.qc", "fail(1)")
        .unwrap_err();
    assert_eq!(
        retained_error.labels()[0].span.source_name.as_deref(),
        Some("virtual://definitions.qc")
    );
}

#[test]
fn runtime_errors_keep_ordered_secondary_quickcoffee_call_sites() {
    const NAME: &str = "virtual://runtime/call-stack.qc";
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
        .eval_named("virtual://definitions.qc", "fail = -> missing")
        .unwrap();
    let cross_eval = retained
        .eval_named("virtual://caller.qc", "fail()")
        .unwrap_err();
    assert_eq!(cross_eval.labels().len(), 2);
    assert_eq!(
        cross_eval.labels()[0].span.source_name.as_deref(),
        Some("virtual://definitions.qc")
    );
    assert_eq!(
        cross_eval.labels()[1].span.source_name.as_deref(),
        Some("virtual://caller.qc")
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
        .compile_program_named("virtual://square.qc", source)
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
