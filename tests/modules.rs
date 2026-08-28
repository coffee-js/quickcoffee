use quickcoffee::{
    CapabilityKey, CapabilityKind, CompileLimits, Context, Engine, ErrorKind,
    MODULE_GRAPH_FINGERPRINT_VERSION, MemoryModuleLoader, ModuleLoader, ModuleSource,
    ResourceLimit, ResourceLimits, RestrictedFileModuleLoader, Runtime, Value,
};
use std::{
    cell::Cell,
    fs,
    path::PathBuf,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

fn module_temp(name: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "quickcoffee-module-{name}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

struct CanonicalModuleLoader {
    canonical_name: &'static str,
    source: &'static str,
}

impl ModuleLoader for CanonicalModuleLoader {
    fn load(&self, _specifier: &str, _referrer: &str) -> Result<ModuleSource, quickcoffee::Error> {
        Ok(ModuleSource::new(self.canonical_name, self.source))
    }
}

#[test]
fn runtime_module_cache_shares_compilation_but_never_evaluation_results() {
    let runtime = Runtime::builder()
        .program_cache_entries(0)
        .module_cache_entries(2)
        .build();
    let entry = runtime
        .compile_module(
            "main",
            "import { value } from 'dependency'\nexport result = value",
        )
        .unwrap();
    let mut loader = MemoryModuleLoader::new();
    loader.insert("dependency", "export value = tick()");
    let calls = Rc::new(Cell::new(0_u64));

    let make_context = || {
        let calls = calls.clone();
        runtime
            .context_builder()
            .native("tick", move |_| {
                let next = calls.get() + 1;
                calls.set(next);
                Ok(Value::from(next as f64))
            })
            .build()
    };
    let mut first = make_context();
    let mut second = make_context();
    assert_eq!(
        first
            .run_module(&entry, &loader)
            .unwrap()
            .get("result")
            .and_then(Value::as_number),
        Some(1.)
    );
    assert_eq!(
        second
            .run_module(&entry, &loader)
            .unwrap()
            .get("result")
            .and_then(Value::as_number),
        Some(2.)
    );
    assert_eq!(calls.get(), 2);

    let shared = runtime.cache_stats();
    assert_eq!(shared.module_entries, 2);
    assert_eq!(shared.module_hits, 1);
    assert_eq!(shared.module_misses, 2);

    runtime
        .compile_module("dependency", "export value = tick() + 0")
        .unwrap();
    assert!(
        runtime
            .compile_module("broken", "export value = if")
            .is_err()
    );
    let invalidated = runtime.cache_stats();
    assert_eq!(invalidated.module_entries, 2);
    assert_eq!(invalidated.module_misses, 4);
    assert_eq!(invalidated.module_evictions, 1);

    let disabled = Runtime::builder()
        .program_cache_entries(0)
        .module_cache_entries(0)
        .build();
    disabled
        .compile_module("answer", "export value = 42")
        .unwrap();
    disabled
        .compile_module("answer", "export value = 42")
        .unwrap();
    assert_eq!(disabled.cache_stats().module_entries, 0);
    assert_eq!(disabled.cache_stats().module_hits, 0);
    assert_eq!(disabled.cache_stats().module_misses, 2);
}

#[test]
fn module_children_inherit_contextual_native_host_state() {
    let entry = Engine::new()
        .compile_module("main", "export value = host_value()")
        .unwrap();
    let mut context = Context::builder()
        .host_state(Cell::new(41_u64))
        .contextual_native("host_value", |call, _| {
            let state = call
                .host_state::<Cell<u64>>()
                .ok_or_else(|| quickcoffee::Error::runtime("missing module host state"))?;
            state.set(state.get() + 1);
            Ok(Value::from(state.get() as f64))
        })
        .build();
    let exports = context
        .run_module(&entry, &MemoryModuleLoader::new())
        .unwrap();
    assert_eq!(exports.get("value").and_then(Value::as_number), Some(42.));
    assert_eq!(context.host_state::<Cell<u64>>().unwrap().get(), 42);
}

#[test]
fn module_children_inherit_typed_capability_handles() {
    let audit = CapabilityKey::<Cell<u64>>::new(CapabilityKind::Logging, "module-audit");
    let entry = Engine::new()
        .compile_module("main", "export value = host_audit()")
        .unwrap();
    let mut context = Context::builder()
        .capability(audit, Cell::new(40_u64))
        .contextual_native("host_audit", move |call, _| {
            let sink = call
                .capability(audit)
                .ok_or_else(|| quickcoffee::Error::runtime("missing module logging capability"))?;
            sink.set(sink.get() + 1);
            Ok(Value::from(sink.get() as f64))
        })
        .build();
    let exports = context
        .run_module(&entry, &MemoryModuleLoader::new())
        .unwrap();
    assert_eq!(exports.get("value").and_then(Value::as_number), Some(41.));
    assert_eq!(context.capability(audit).unwrap().get(), 41);
}

#[test]
fn compile_limits_bound_module_bytecode_and_preflight_graphs_before_execution() {
    let entry_source =
        "import { left } from 'left'\nimport { right } from 'right'\nexport total = left + right";
    let left_source = "export left = tick()";
    let right_source = "export right = tick()";
    let ordinary = Engine::new().compile_module("main", entry_source).unwrap();
    assert!(ordinary.instruction_count() > 1);
    let error = Engine::new()
        .with_compile_limits(
            CompileLimits::default()
                .with_max_bytecode_instructions(ordinary.instruction_count() - 1),
        )
        .compile_module("main", entry_source)
        .unwrap_err();
    assert_eq!(
        error.resource_limit(),
        Some(ResourceLimit::BytecodeInstructions)
    );

    let mut loader = MemoryModuleLoader::new();
    loader.insert("left", left_source);
    loader.insert("right", right_source);
    let module_limits = CompileLimits::default().with_max_module_graph_modules(2);
    let runtime = Runtime::builder().compile_limits(module_limits).build();
    let entry = runtime.compile_module("main", entry_source).unwrap();
    let calls = Rc::new(Cell::new(0_u64));
    let observed = calls.clone();
    let mut context = runtime
        .context_builder()
        .native("tick", move |_| {
            observed.set(observed.get() + 1);
            Ok(Value::from(1_f64))
        })
        .build();
    let error = context.run_module(&entry, &loader).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Resource);
    assert_eq!(
        error.resource_limit(),
        Some(ResourceLimit::ModuleGraphModules)
    );
    assert_eq!(calls.get(), 0);
    let error = Engine::new()
        .with_compile_limits(module_limits)
        .fingerprint_module_graph(&entry, &loader)
        .unwrap_err();
    assert_eq!(
        error.resource_limit(),
        Some(ResourceLimit::ModuleGraphModules)
    );

    let graph_bytes = entry_source.len() + left_source.len() + right_source.len();
    let byte_limits = CompileLimits::default().with_max_module_graph_source_bytes(graph_bytes - 1);
    let runtime = Runtime::builder().compile_limits(byte_limits).build();
    let entry = runtime.compile_module("main", entry_source).unwrap();
    let mut context = runtime.new_context();
    let error = context.run_module(&entry, &loader).unwrap_err();
    assert_eq!(
        error.resource_limit(),
        Some(ResourceLimit::ModuleGraphSourceBytes)
    );
    assert_eq!(context.last_execution().instructions, 0);
    let error = Engine::new()
        .with_compile_limits(byte_limits)
        .fingerprint_module_graph(&entry, &loader)
        .unwrap_err();
    assert_eq!(
        error.resource_limit(),
        Some(ResourceLimit::ModuleGraphSourceBytes)
    );

    let exact = CompileLimits::default().with_max_module_graph_source_bytes(graph_bytes);
    let runtime = Runtime::builder().compile_limits(exact).build();
    let entry = runtime.compile_module("main", entry_source).unwrap();
    let mut context = runtime
        .context_builder()
        .native("tick", |_| Ok(Value::from(1_f64)))
        .build();
    assert_eq!(
        context
            .run_module(&entry, &loader)
            .unwrap()
            .get("total")
            .and_then(Value::as_number),
        Some(2_f64)
    );
}

#[test]
fn restricted_file_loader_bounds_source_before_full_read() {
    let root = module_temp("source-limit");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("main.coffee"), "export value = true").unwrap();
    let loader = RestrictedFileModuleLoader::new(&root)
        .unwrap()
        .with_max_source_bytes(4);
    let error = loader.load_entry("main").unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Resource);
    assert_eq!(error.resource_limit(), Some(ResourceLimit::SourceBytes));
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("main.coffee")
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn module_graph_fingerprints_are_stable_versioned_and_dependency_sensitive() {
    assert_eq!(MODULE_GRAPH_FINGERPRINT_VERSION, 1);
    let engine = Engine::new();
    let entry = engine
        .compile_module(
            "app/main",
            "import { left } from 'left'\nimport { right } from 'right'\nexport total = left + right",
        )
        .unwrap();
    let mut forward = MemoryModuleLoader::new();
    forward.insert("left", "export left = 20");
    forward.insert("right", "export right = 22");
    let mut reverse = MemoryModuleLoader::new();
    reverse.insert("right", "export right = 22");
    reverse.insert("left", "export left = 20");

    let fingerprint = engine.fingerprint_module_graph(&entry, &forward).unwrap();
    assert_eq!(fingerprint, 0xff97_10b9_ddd9_a0d1);
    assert_ne!(fingerprint, 0);
    assert_eq!(
        fingerprint,
        engine.fingerprint_module_graph(&entry, &forward).unwrap()
    );
    assert_eq!(
        fingerprint,
        engine.fingerprint_module_graph(&entry, &reverse).unwrap()
    );

    let mut changed_source = forward.clone();
    changed_source.insert(
        "right",
        "# source-only cache invalidation\nexport right = 22",
    );
    let original_dependency = engine.compile_module("right", "export right = 22").unwrap();
    let changed_dependency = engine
        .compile_module(
            "right",
            "# source-only cache invalidation\nexport right = 22",
        )
        .unwrap();
    assert_eq!(
        original_dependency.fingerprint(),
        changed_dependency.fingerprint()
    );
    assert_ne!(
        fingerprint,
        engine
            .fingerprint_module_graph(&entry, &changed_source)
            .unwrap()
    );

    let renamed_entry = engine
        .compile_module(
            "app/renamed",
            "import { left } from 'left'\nimport { right } from 'right'\nexport total = left + right",
        )
        .unwrap();
    assert_ne!(
        fingerprint,
        engine
            .fingerprint_module_graph(&renamed_entry, &forward)
            .unwrap()
    );

    let changed_import = engine
        .compile_module(
            "app/main",
            "import { left as first } from 'left'\nimport { right } from 'right'\nexport total = first + right",
        )
        .unwrap();
    assert_ne!(
        fingerprint,
        engine
            .fingerprint_module_graph(&changed_import, &forward)
            .unwrap()
    );

    let changed_export = engine
        .compile_module(
            "app/main",
            "import { left } from 'left'\nimport { right } from 'right'\nexport result = left + right",
        )
        .unwrap();
    assert_ne!(
        fingerprint,
        engine
            .fingerprint_module_graph(&changed_export, &forward)
            .unwrap()
    );

    let edge_entry = engine
        .compile_module(
            "entry",
            "import { value } from 'alias'\nexport value = value",
        )
        .unwrap();
    let first_edge = engine
        .fingerprint_module_graph(
            &edge_entry,
            &CanonicalModuleLoader {
                canonical_name: "canonical/first",
                source: "export value = 42",
            },
        )
        .unwrap();
    let second_edge = engine
        .fingerprint_module_graph(
            &edge_entry,
            &CanonicalModuleLoader {
                canonical_name: "canonical/second",
                source: "export value = 42",
            },
        )
        .unwrap();
    assert_ne!(first_edge, second_edge);
}

#[test]
fn module_graph_fingerprints_reject_missing_cycles_and_inconsistent_canonical_sources() {
    let engine = Engine::new();
    let missing = engine
        .compile_module(
            "main",
            "import { value } from 'missing'\nexport value = value",
        )
        .unwrap();
    let error = engine
        .fingerprint_module_graph(&missing, &MemoryModuleLoader::new())
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Runtime);
    assert_eq!(error.message(), "module not found: missing");

    let missing_export = engine
        .compile_module(
            "main",
            "import { absent } from 'dependency'\nexport value = absent",
        )
        .unwrap();
    let mut missing_export_loader = MemoryModuleLoader::new();
    missing_export_loader.insert("dependency", "export present = 42");
    let error = engine
        .fingerprint_module_graph(&missing_export, &missing_export_loader)
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Runtime);
    assert_eq!(error.message(), "module dependency does not export absent");

    let cycle = engine
        .compile_module("a", "import { value } from 'b'\nexport value = value")
        .unwrap();
    let mut cycle_loader = MemoryModuleLoader::new();
    cycle_loader.insert("a", "import { value } from 'b'\nexport value = value");
    cycle_loader.insert("b", "import { value } from 'a'\nexport value = value");
    let error = engine
        .fingerprint_module_graph(&cycle, &cycle_loader)
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Runtime);
    assert_eq!(error.message(), "circular module dependency: a -> b -> a");

    struct InconsistentLoader;
    impl ModuleLoader for InconsistentLoader {
        fn load(
            &self,
            specifier: &str,
            _referrer: &str,
        ) -> Result<ModuleSource, quickcoffee::Error> {
            Ok(ModuleSource::new(
                "canonical/shared",
                if specifier == "first" {
                    "export value = 1"
                } else {
                    "export value = 2"
                },
            ))
        }
    }
    let inconsistent = engine
        .compile_module(
            "main",
            "import { value as first } from 'first'\nimport { value as second } from 'second'\nexport total = first + second",
        )
        .unwrap();
    let error = engine
        .fingerprint_module_graph(&inconsistent, &InconsistentLoader)
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Runtime);
    assert_eq!(
        error.message(),
        "module canonical name resolved to inconsistent source: canonical/shared"
    );
}

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
    assert!(stats.managed_objects_allocated > 0);
    assert!(stats.managed_bytes_allocated > 0);
}

#[test]
fn module_exports_are_counted_only_after_the_host_retains_them_in_context() {
    let engine = Engine::new();
    let module = engine
        .compile_module("memory", "export payload = ['coffee', 'beans']")
        .unwrap();
    let context_before = Context::new().retained_memory();
    let mut context = Context::new();
    let exports = context
        .run_module(&module, &MemoryModuleLoader::new())
        .unwrap();
    assert_eq!(context.retained_memory(), context_before);

    context.set_global("payload", exports.get("payload").unwrap().clone());
    let retained = context.retained_memory();
    assert!(retained.objects > context_before.objects);
    assert!(retained.bytes > context_before.bytes);
}

#[test]
fn module_execution_and_host_roots_need_explicit_high_water_samples() {
    let engine = Engine::new();
    let module = engine
        .compile_module("memory", "export payload = ['coffee', 'beans']")
        .unwrap();
    let mut context = Context::new();
    let initial_high_water = context.retained_memory_high_water();
    let exports = context
        .run_module(&module, &MemoryModuleLoader::new())
        .unwrap();
    assert_eq!(context.retained_memory_high_water(), initial_high_water);

    context.set_global("payload", exports.get("payload").unwrap().clone());
    assert_eq!(context.retained_memory_high_water(), initial_high_water);
    let snapshot = context.sample_retained_memory();
    assert!(snapshot.objects > initial_high_water.objects);
    assert!(snapshot.bytes > initial_high_water.bytes);
    assert_eq!(context.retained_memory_high_water(), snapshot);
}

#[test]
fn module_children_inherit_retained_memory_commit_limits() {
    let engine = Engine::new();
    let module = engine
        .compile_module("memory", "export payload = ['coffee']")
        .unwrap();
    let mut context = Context::new()
        .with_resource_limits(ResourceLimits::default().with_max_retained_managed_objects(2));
    let error = context
        .run_module(&module, &MemoryModuleLoader::new())
        .unwrap_err();
    assert_eq!(
        error.resource_limit(),
        Some(ResourceLimit::RetainedManagedObjects)
    );
    assert_eq!(context.retained_memory().objects, 1);
}

#[test]
fn module_graphs_share_one_transient_managed_allocation_budget() {
    let engine = Engine::new();
    let main = engine
        .compile_module(
            "main",
            "import { dependency } from 'dependency'\nexport result = concat('', dependency)",
        )
        .unwrap();
    let mut loader = MemoryModuleLoader::new();
    loader.insert("dependency", "export dependency = concat('', 'coffee')");

    let mut baseline = Context::new();
    let exports = baseline.run_module(&main, &loader).unwrap();
    assert_eq!(
        exports.get("result").and_then(Value::as_str),
        Some("coffee")
    );
    let expected = baseline.last_execution();
    assert_eq!(expected.managed_objects_allocated, 2);
    assert_eq!(expected.managed_bytes_allocated, 12);

    let exact_limits = ResourceLimits::default()
        .with_max_transient_managed_objects(expected.managed_objects_allocated)
        .with_max_transient_managed_bytes(expected.managed_bytes_allocated);
    let mut exact = Context::new().with_resource_limits(exact_limits);
    assert!(exact.run_module(&main, &loader).is_ok());
    assert_eq!(exact.last_execution(), expected);

    let mut limited = Context::new()
        .with_resource_limits(ResourceLimits::default().with_max_transient_managed_objects(1));
    let error = limited.run_module(&main, &loader).unwrap_err();
    assert_eq!(
        error.resource_limit(),
        Some(ResourceLimit::TransientManagedObjects)
    );
    assert_eq!(limited.last_execution().managed_objects_allocated, 2);
}

#[test]
fn modules_export_classes_without_exposing_their_receiver_state() {
    let engine = Engine::new();
    let main = engine
        .compile_module(
            "main",
            "import { Point } from 'geometry'\npoint = new Point(42)\nexport result = point.value()",
        )
        .unwrap();
    let mut loader = MemoryModuleLoader::new();
    loader.insert(
        "geometry",
        "class Point\n  constructor: (@x) ->\n  value: -> @x\nexport { Point }",
    );

    let exports = Context::new().run_module(&main, &loader).unwrap();
    assert_eq!(exports.get("result").and_then(Value::as_number), Some(42.));
}

#[test]
fn modules_can_privately_extend_imported_classes() {
    let engine = Engine::new();
    let main = engine
        .compile_module(
            "main",
            "import { Base } from 'model'\nclass Child extends Base\n  constructor: (value) -> super(value + 1)\n  score: -> super() + 1\nexport result = new Child(40).score()",
        )
        .unwrap();
    let mut loader = MemoryModuleLoader::new();
    loader.insert(
        "model",
        "class Base\n  constructor: (@value) ->\n  score: -> @value\nexport { Base }",
    );

    let exports = Context::new().run_module(&main, &loader).unwrap();
    assert_eq!(exports.get("result").and_then(Value::as_number), Some(42.));
}

#[test]
fn modules_can_export_receiver_bound_callbacks_without_exporting_receivers() {
    let engine = Engine::new();
    let main = engine
        .compile_module(
            "main",
            "import { callback } from 'counter'\nexport result = [callback(), callback()]",
        )
        .unwrap();
    let mut loader = MemoryModuleLoader::new();
    loader.insert(
        "counter",
        "class Counter\n  constructor: (@value) ->\n  callback: ->\n    =>\n      @value = @value + 1\n      @value\nexport callback = new Counter(40).callback()",
    );

    let exports = Context::new().run_module(&main, &loader).unwrap();
    assert_eq!(exports.get("result").unwrap().to_string(), "[41, 42]");
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
    assert_eq!(error.labels().len(), 2);
    assert_eq!(
        error.labels()[1].kind,
        quickcoffee::DiagnosticLabelKind::Secondary
    );
    assert_eq!(
        error.labels()[1].message.as_deref(),
        Some("called from here")
    );
    assert_eq!(error.labels()[1].span.source_name.as_deref(), Some("main"));
    assert_eq!(error.labels()[1].span.start.line, 2);
}

#[test]
fn modules_preserve_structured_error_values_and_trusted_labels() {
    let main = Engine::new()
        .compile_module(
            "main",
            "import { fail } from 'dependency'\nexport code = try fail() catch problem then problem.code",
        )
        .unwrap();
    let mut loader = MemoryModuleLoader::new();
    loader.insert(
        "dependency",
        "fail = -> throw error('dependency.failed', 'failed', {retry: false})\nexport { fail }",
    );
    let exports = Context::new().run_module(&main, &loader).unwrap();
    assert_eq!(
        exports.get("code").and_then(Value::as_str),
        Some("dependency.failed")
    );

    let uncaught = Engine::new()
        .compile_module(
            "uncaught",
            "import { fail } from 'dependency'\nexport result = fail()",
        )
        .unwrap();
    let error = Context::new().run_module(&uncaught, &loader).unwrap_err();
    assert_eq!(error.script_error().unwrap().code(), "dependency.failed");
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("dependency")
    );
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

    loader.insert("a", "loop 1\nexport value = 1");
    let error = Context::new()
        .with_fuel(8)
        .run_module(&main, &loader)
        .unwrap_err();
    assert_eq!(error.resource_limit(), Some(ResourceLimit::Fuel));
}

#[test]
fn module_children_inherit_json_resource_policy() {
    let engine = Engine::new();
    let main = engine
        .compile_module(
            "main",
            "import { payload } from 'dependency'\nexport payload = payload",
        )
        .unwrap();
    let mut loader = MemoryModuleLoader::new();
    loader.insert("dependency", "export payload = parse_json('[0]')");
    let limits = ResourceLimits::default().with_max_json_values(1);
    let error = Context::new()
        .with_resource_limits(limits)
        .run_module(&main, &loader)
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Resource);
    assert_eq!(error.resource_limit(), Some(ResourceLimit::JsonValueCount));
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("dependency")
    );
}

#[test]
fn module_children_inherit_exact_numeric_resource_policy() {
    let engine = Engine::new();
    let main = engine
        .compile_module(
            "main",
            "import { payload } from 'dependency'\nexport payload = payload",
        )
        .unwrap();
    let mut loader = MemoryModuleLoader::new();
    loader.insert("dependency", "export payload = 8n");
    let limits = ResourceLimits::default().with_max_integer_bits(3);
    let error = Context::new()
        .with_resource_limits(limits)
        .run_module(&main, &loader)
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Resource);
    assert_eq!(error.resource_limit(), Some(ResourceLimit::IntegerBits));
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("dependency")
    );
}

#[test]
fn module_children_inherit_collection_operation_resource_policy() {
    let engine = Engine::new();
    let main = engine
        .compile_module(
            "main",
            "import { payload } from 'dependency'\nexport payload = payload",
        )
        .unwrap();
    let mut loader = MemoryModuleLoader::new();
    loader.insert("dependency", "export payload = sort([3, 2, 1])");
    let limits = ResourceLimits::default().with_max_collection_operation_items(2);
    let error = Context::new()
        .with_resource_limits(limits)
        .run_module(&main, &loader)
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Resource);
    assert_eq!(
        error.resource_limit(),
        Some(ResourceLimit::CollectionOperationItems)
    );
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("dependency")
    );
}

#[test]
fn module_children_inherit_text_operation_resource_policy() {
    let engine = Engine::new();
    let main = engine
        .compile_module(
            "main",
            "import { payload } from 'dependency'\nexport payload = payload",
        )
        .unwrap();
    let mut loader = MemoryModuleLoader::new();
    loader.insert(
        "dependency",
        "export payload = replace_all('banana', 'a', 'x')",
    );
    let limits = ResourceLimits::default().with_max_text_operation_bytes(5);
    let error = Context::new()
        .with_resource_limits(limits)
        .run_module(&main, &loader)
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Resource);
    assert_eq!(
        error.resource_limit(),
        Some(ResourceLimit::TextOperationBytes)
    );
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("dependency")
    );
}

#[test]
fn module_children_inherit_general_value_resource_policy() {
    let engine = Engine::new();
    let main = engine
        .compile_module(
            "main",
            "import { payload } from 'dependency'\nexport payload = payload",
        )
        .unwrap();
    let mut loader = MemoryModuleLoader::new();
    loader.insert("dependency", "export payload = [1, 2, 3]");
    let limits = ResourceLimits::default().with_max_array_items(2);
    let error = Context::new()
        .with_resource_limits(limits)
        .run_module(&main, &loader)
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Resource);
    assert_eq!(error.resource_limit(), Some(ResourceLimit::ArrayItems));
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("dependency")
    );
}

#[test]
fn module_directives_are_not_accepted_by_single_file_compilation() {
    let error = Engine::new()
        .compile_program_named("virtual://single.coffee", "value = 1\nexport answer = 42")
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Verify);
    assert_eq!(
        error.to_string(),
        "verify error: module directives require Engine::compile_module"
    );
    let span = &error.labels()[0].span;
    assert_eq!(span.source_name.as_deref(), Some("virtual://single.coffee"));
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

#[test]
fn restricted_file_loader_resolves_nested_relative_modules() {
    let root = module_temp("relative");
    fs::create_dir_all(root.join("app/lib")).unwrap();
    fs::create_dir_all(root.join("shared")).unwrap();
    fs::write(
        root.join("app/main.coffee"),
        "import { double } from './lib/math'\nimport { base } from '../shared/value.coffee'\nexport result = double(base)",
    )
    .unwrap();
    fs::write(
        root.join("app/lib/math.coffee"),
        "export double = (value) -> value * 2",
    )
    .unwrap();
    fs::write(root.join("shared/value.coffee"), "export base = 21").unwrap();

    let loader = RestrictedFileModuleLoader::new(&root).unwrap();
    let source = loader.load_entry("app/main").unwrap();
    assert_eq!(source.name(), "app/main.coffee");
    let main = Engine::new()
        .compile_module(source.name(), source.source())
        .unwrap();
    let exports = Context::new().run_module(&main, &loader).unwrap();
    assert_eq!(exports.get("result").and_then(Value::as_number), Some(42.));

    let normalized = loader.load("./lib/../lib/math", "app/main.coffee").unwrap();
    assert_eq!(normalized.name(), "app/lib/math.coffee");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn restricted_file_module_graph_fingerprints_track_canonical_sources() {
    let root = module_temp("fingerprint");
    fs::create_dir_all(root.join("app/lib")).unwrap();
    fs::write(
        root.join("app/main.coffee"),
        "import { value } from './lib/../lib/value.litcoffee'\nexport result = value",
    )
    .unwrap();
    fs::write(
        root.join("app/lib/value.litcoffee"),
        "# Dependency documentation\n\n    export value = 42\n",
    )
    .unwrap();

    let engine = Engine::new();
    let loader = RestrictedFileModuleLoader::new(&root).unwrap();
    let source = loader.load_entry("app/main").unwrap();
    let entry = engine
        .compile_module(source.name(), source.source())
        .unwrap();
    let first = engine.fingerprint_module_graph(&entry, &loader).unwrap();
    assert_eq!(
        first,
        engine.fingerprint_module_graph(&entry, &loader).unwrap()
    );

    fs::write(
        root.join("app/lib/value.litcoffee"),
        "# Changed dependency documentation\n\n    export value = 42\n",
    )
    .unwrap();
    let changed = engine.fingerprint_module_graph(&entry, &loader).unwrap();
    assert_ne!(first, changed);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn restricted_file_loader_executes_explicit_litcoffee_modules() {
    let root = module_temp("literate");
    fs::create_dir_all(root.join("app")).unwrap();
    fs::write(
        root.join("app/main.litcoffee"),
        "# Literate entry\n\n    export answer = 42\n",
    )
    .unwrap();

    let loader = RestrictedFileModuleLoader::new(&root).unwrap();
    let source = loader.load_entry("app/main.litcoffee").unwrap();
    assert_eq!(source.name(), "app/main.litcoffee");
    let module = Engine::new()
        .compile_module(source.name(), source.source())
        .unwrap();
    let exports = Context::new().run_module(&module, &loader).unwrap();
    assert_eq!(exports.get("answer").and_then(Value::as_number), Some(42.));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn restricted_file_loader_rejects_ambiguous_and_escaping_paths() {
    let root = module_temp("invalid");
    fs::create_dir_all(root.join("app")).unwrap();
    fs::write(root.join("app/main.coffee"), "export value = 1").unwrap();
    let loader = RestrictedFileModuleLoader::new(&root).unwrap();

    for specifier in [
        "package",
        "/absolute",
        "./windows\\module",
        "./wrong.txt",
        "./legacy.qc",
    ] {
        let error = loader.load(specifier, "app/main.coffee").unwrap_err();
        assert_eq!(
            error.message(),
            format!("invalid module specifier: {specifier}")
        );
    }
    let error = loader.load("../../outside", "app/main.coffee").unwrap_err();
    assert_eq!(
        error.message(),
        "module path escapes configured root: ../../outside"
    );
    let error = loader.load("./missing", "app/main.coffee").unwrap_err();
    assert_eq!(error.message(), "module not found: app/missing.coffee");
    let error = loader.load("./main", "../app/main.coffee").unwrap_err();
    assert_eq!(
        error.message(),
        "invalid module referrer: ../app/main.coffee"
    );
    let error = loader.load_entry("../outside").unwrap_err();
    assert_eq!(error.message(), "invalid module entry: ../outside");
    let error = loader.load_entry("app/main.txt").unwrap_err();
    assert_eq!(error.message(), "invalid module entry: app/main.txt");
    fs::write(root.join("app/binary.coffee"), [0xff]).unwrap();
    let error = loader.load_entry("app/binary").unwrap_err();
    assert_eq!(
        error.message(),
        "module source is not readable UTF-8: app/binary.coffee"
    );
    fs::create_dir(root.join("app/directory.coffee")).unwrap();
    let error = loader.load_entry("app/directory").unwrap_err();
    assert_eq!(
        error.message(),
        "invalid module target: app/directory.coffee"
    );
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn restricted_file_loader_canonicalizes_aliases_and_rejects_symlink_escape() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let base = module_temp("symlink");
    let root = base.join("root");
    fs::create_dir_all(root.join("real")).unwrap();
    fs::write(root.join("real/module.coffee"), "export value = 42").unwrap();
    fs::write(base.join("outside.coffee"), "export value = 0").unwrap();
    fs::write(root.join("real:module.coffee"), "export value = 1").unwrap();
    fs::write(root.join("real\\module.coffee"), "export value = 2").unwrap();
    symlink("real/module.coffee", root.join("alias.coffee")).unwrap();
    symlink(base.join("outside.coffee"), root.join("escape.coffee")).unwrap();
    symlink("real:module.coffee", root.join("colon-alias.coffee")).unwrap();
    symlink("real\\module.coffee", root.join("backslash-alias.coffee")).unwrap();

    let loader = RestrictedFileModuleLoader::new(&root).unwrap();
    let alias = loader.load_entry("alias").unwrap();
    assert_eq!(alias.name(), "real/module.coffee");
    let real = loader.load_entry("real/module").unwrap();
    let engine = Engine::new();
    let alias_module = engine.compile_module(alias.name(), alias.source()).unwrap();
    let real_module = engine.compile_module(real.name(), real.source()).unwrap();
    assert_eq!(
        engine
            .fingerprint_module_graph(&alias_module, &loader)
            .unwrap(),
        engine
            .fingerprint_module_graph(&real_module, &loader)
            .unwrap()
    );
    let error = loader.load_entry("escape").unwrap_err();
    assert_eq!(
        error.message(),
        "module path escapes configured root: escape.coffee"
    );
    for alias in ["colon-alias", "backslash-alias"] {
        let error = loader.load_entry(alias).unwrap_err();
        assert_eq!(
            error.message(),
            format!("invalid module target: {alias}.coffee")
        );
    }
    let locked = root.join("locked.coffee");
    fs::write(&locked, "export value = 3").unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let error = loader.load_entry("locked").unwrap_err();
    assert_eq!(
        error.message(),
        "module source is not readable: locked.coffee"
    );
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o600)).unwrap();
    let _ = fs::remove_dir_all(base);
}
