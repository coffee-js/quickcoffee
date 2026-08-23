use quickcoffee::{
    Context, Engine, ErrorKind, MemoryModuleLoader, ModuleLoader, ResourceLimit, Value,
};

#[test]
fn named_static_imports_and_exports_keep_module_globals_private() {
    let engine = Engine::new();
    let main = engine
        .compile_module(
            "main",
            "import { answer, double } from 'math'\nhidden = 1\nexport result = double(answer)",
        )
        .unwrap();
    let mut loader = MemoryModuleLoader::new();
    loader.insert(
        "math",
        "answer = 21\ntwice = (value) -> value * 2\nexport { answer, twice as double }",
    );

    let mut context = Context::new();
    let exports = context.run_module(&main, &loader).unwrap();
    assert_eq!(exports.get("result").and_then(Value::as_number), Some(42.));
    assert_eq!(exports.len(), 1);
    assert!(context.get_global("hidden").is_none());
    assert!(context.get_global("answer").is_none());
    let stats = context.last_execution();
    assert!(stats.name_loads > 0);
    assert!(stats.calls > 0);
    assert!(stats.value_allocations > 0);
    assert!(stats.environment_allocations > 0);
}

#[test]
fn module_loading_caches_dependencies_and_reports_missing_names() {
    let engine = Engine::new();
    let main = engine
        .compile_module(
            "main",
            "import { value } from 'shared'\nimport { value as again } from 'shared'\nexport sum = value + again",
        )
        .unwrap();
    let mut loader = MemoryModuleLoader::new();
    loader.insert("shared", "export value = 21");
    let exports = Context::new().run_module(&main, &loader).unwrap();
    assert_eq!(exports.get("sum").and_then(Value::as_number), Some(42.));

    let missing = engine
        .compile_module(
            "missing",
            "import { absent } from 'shared'\nexport value = absent",
        )
        .unwrap();
    let error = Context::new().run_module(&missing, &loader).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Runtime);
    assert!(error.message().contains("does not export absent"));
}

#[test]
fn runtime_errors_in_dependency_functions_keep_the_dependency_name() {
    let main = Engine::new()
        .compile_module(
            "main",
            "import { fail } from 'dependency'\nexport result = fail(1)",
        )
        .unwrap();
    let mut loader = MemoryModuleLoader::new();
    loader.insert(
        "dependency",
        "fail = (value) -> value + 'x'\nexport { fail }",
    );
    let error = Context::new().run_module(&main, &loader).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Runtime);
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("dependency")
    );
    assert_eq!(error.labels()[0].span.start.line, 1);
}

#[test]
fn module_cycles_are_rejected_and_resources_cover_dependencies() {
    let engine = Engine::new();
    let main = engine
        .compile_module("main", "import { value } from 'a'\nexport value = value")
        .unwrap();
    let mut loader = MemoryModuleLoader::new();
    loader.insert("a", "import { value } from 'b'\nexport value = value");
    loader.insert("b", "import { value } from 'a'\nexport value = value");
    let error = Context::new().run_module(&main, &loader).unwrap_err();
    assert!(
        error
            .message()
            .contains("circular module dependency: main -> a -> b -> a")
    );

    loader.insert("a", "loop 1");
    let error = Context::new()
        .with_fuel(8)
        .run_module(&main, &loader)
        .unwrap_err();
    assert_eq!(error.resource_limit(), Some(ResourceLimit::Fuel));
}

#[test]
fn module_directives_are_not_accepted_by_single_file_compilation() {
    let error = Engine::new()
        .compile_program_named("virtual://single.qc", "value = 1\nexport answer = 42")
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Verify);
    assert_eq!(
        error.to_string(),
        "verify error: module directives require Engine::compile_module"
    );
    let span = &error.labels()[0].span;
    assert_eq!(span.source_name.as_deref(), Some("virtual://single.qc"));
    assert_eq!(span.start.line, 2);
    assert_eq!(span.start.column, Some(1));
    assert_eq!(
        span.end,
        Some(quickcoffee::SourcePosition {
            line: 2,
            column: Some(7),
        })
    );
}

#[test]
fn module_parse_errors_carry_the_host_canonical_name() {
    let error = Engine::new()
        .compile_module("pkg://rules/../invoice", "value = 1\n@")
        .unwrap_err();
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("pkg://rules/../invoice")
    );
}

#[test]
fn duplicate_module_exports_point_to_the_later_directive() {
    let error = Engine::new()
        .compile_module(
            "pkg://rules/invoice",
            "export answer = 41\nexport answer = 42",
        )
        .unwrap_err();
    assert_eq!(error.message(), "duplicate module export: answer");
    let span = &error.labels()[0].span;
    assert_eq!(span.source_name.as_deref(), Some("pkg://rules/invoice"));
    assert_eq!(span.start.line, 2);
    assert_eq!(span.start.column, Some(1));
    assert_eq!(
        span.end,
        Some(quickcoffee::SourcePosition {
            line: 2,
            column: Some(7),
        })
    );
}

#[test]
fn memory_loader_only_resolves_exact_names() {
    let loader = MemoryModuleLoader::new();
    let error = loader.load("./missing", "main").unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Runtime);
    assert_eq!(error.message(), "module not found: ./missing");
}
