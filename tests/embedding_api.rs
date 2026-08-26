use quickcoffee::{
    CancellationToken, Context, Decimal, DiagnosticLabelKind, Engine, Error, ErrorKind, Integer,
    IntoValue, ResourceLimit, ResourceLimits, TryFromValue, Value, ValueKind,
};
use std::collections::BTreeMap;

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
            "virtual://broken-class.qc",
            "class Broken\n  fail: -> missing\nnew Broken().fail()",
        )
        .unwrap();
    let error = Context::new().run_program(&program).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Runtime);
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("virtual://broken-class.qc")
    );
    assert_eq!(error.labels()[0].span.start.line, 2);
    assert!(error.labels().iter().any(|label| {
        label.kind == DiagnosticLabelKind::Secondary
            && label.span.source_name.as_deref() == Some("virtual://broken-class.qc")
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
            "virtual://limits.qc",
            "try parse_json('[0]') catch ignored then 42",
        )
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Resource);
    assert_eq!(error.resource_limit(), Some(ResourceLimit::JsonValueCount));
    assert!(error.message().contains("value count exceeds 1"));
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("virtual://limits.qc")
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
            "virtual://collection-limits.qc",
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
        Some("virtual://collection-limits.qc")
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
            "virtual://concat-limits.qc",
            "try concat(prefix, suffix) catch ignored then 'caught'",
        )
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Resource);
    assert_eq!(error.resource_limit(), Some(ResourceLimit::StringBytes));
    assert!(error.message().contains("string exceeds 5 bytes"));
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("virtual://concat-limits.qc")
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
            "virtual://value-limits.qc",
            "try items = [1, 2, 3] catch ignored then [0]",
        )
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Resource);
    assert_eq!(error.resource_limit(), Some(ResourceLimit::ArrayItems));
    assert!(error.message().contains("array exceeds 2 items"));
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("virtual://value-limits.qc")
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
        .eval_named("virtual://numeric-limits.qc", "host_integer")
        .unwrap_err();
    assert_eq!(error.resource_limit(), Some(ResourceLimit::IntegerBits));
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("virtual://numeric-limits.qc")
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
