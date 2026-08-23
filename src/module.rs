use crate::{Context, Engine, Error, ExecutionStats, Program, Value, bytecode, parser};
use std::collections::{BTreeMap, BTreeSet};

/// Source returned by an embedding host for one named QuickCoffee module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleSource {
    name: String,
    source: String,
}
impl ModuleSource {
    /// Creates module source with its host-normalized canonical name.
    pub fn new(name: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            source: source.into(),
        }
    }
    /// Returns the host-normalized canonical module name.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Returns the UTF-8 QuickCoffee source.
    pub fn source(&self) -> &str {
        &self.source
    }
}

/// Host-controlled module source resolver.
///
/// The engine never reads the file system or network. The host receives the
/// literal module specifier and importing module's canonical name, then returns
/// a canonical source name. This first module-core slice accepts only exact
/// named imports and rejects circular dependencies deterministically.
pub trait ModuleLoader {
    /// Resolves a literal module `specifier` requested from `referrer`.
    fn load(&self, specifier: &str, referrer: &str) -> Result<ModuleSource, Error>;
}

/// An in-memory exact-name loader for embedding tests and small applications.
#[derive(Clone, Debug, Default)]
pub struct MemoryModuleLoader {
    sources: BTreeMap<String, String>,
}
impl MemoryModuleLoader {
    /// Creates an empty in-memory module loader.
    pub fn new() -> Self {
        Self::default()
    }
    /// Adds or replaces source under an exact canonical module name.
    pub fn insert(&mut self, name: impl Into<String>, source: impl Into<String>) {
        self.sources.insert(name.into(), source.into());
    }
}
impl ModuleLoader for MemoryModuleLoader {
    fn load(&self, specifier: &str, _referrer: &str) -> Result<ModuleSource, Error> {
        let Some(source) = self.sources.get(specifier) else {
            return Err(Error::runtime(format!("module not found: {specifier}")));
        };
        Ok(ModuleSource::new(specifier, source))
    }
}

/// Compiled static imports and named exports for one QuickCoffee module.
#[derive(Clone, Debug)]
pub struct Module {
    name: String,
    program: Program,
    imports: Vec<(Vec<(String, String)>, String)>,
    exports: Vec<(String, String)>,
}
impl Module {
    /// Returns this module's canonical name.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Returns a stable fingerprint of the executable module body.
    pub fn fingerprint(&self) -> u64 {
        self.program.fingerprint()
    }
}

/// Immutable named values exported by a successfully evaluated module.
#[derive(Clone, Debug, Default)]
pub struct ModuleExports(BTreeMap<String, Value>);
impl ModuleExports {
    /// Returns a named export, if this module declared it.
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.0.get(name)
    }
    /// Returns the number of named exports.
    pub fn len(&self) -> usize {
        self.0.len()
    }
    /// Reports whether this module exports no names.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    /// Iterates over public names and their immutable values in lexical order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.0.iter().map(|(name, value)| (name.as_str(), value))
    }
}

impl Engine {
    /// Compiles one module with static named import/export directives.
    pub fn compile_module(&self, name: impl Into<String>, source: &str) -> Result<Module, Error> {
        let syntax = parser::parse_module(source)?;
        let mut public_names = BTreeSet::new();
        for (public, _) in &syntax.exports {
            if !public_names.insert(public.clone()) {
                return Err(Error::parse(format!("duplicate module export: {public}")));
            }
        }
        let program: Program = bytecode::compile(&syntax.body)?.into();
        program.verify()?;
        Ok(Module {
            name: name.into(),
            program,
            imports: syntax.imports,
            exports: syntax.exports,
        })
    }
}

impl Context {
    /// Loads dependencies through `loader`, runs `module` in a private module
    /// environment, and returns only its named exports.
    pub fn run_module(
        &mut self,
        module: &Module,
        loader: &dyn ModuleLoader,
    ) -> Result<ModuleExports, Error> {
        let mut cache = BTreeMap::new();
        let mut active = Vec::new();
        let mut fuel = self.fuel();
        let mut stats = ExecutionStats::default();
        let result = execute_module(
            self,
            module,
            loader,
            &mut cache,
            &mut active,
            &mut fuel,
            &mut stats,
        );
        if stats.instructions > 0 {
            stats.fuel_remaining = fuel;
            self.set_execution_stats(stats);
        }
        result
    }
}

fn execute_module(
    host: &Context,
    module: &Module,
    loader: &dyn ModuleLoader,
    cache: &mut BTreeMap<String, ModuleExports>,
    active: &mut Vec<String>,
    fuel: &mut u64,
    stats: &mut ExecutionStats,
) -> Result<ModuleExports, Error> {
    if let Some(exports) = cache.get(&module.name) {
        return Ok(exports.clone());
    }
    if active.iter().any(|name| name == &module.name) {
        let mut cycle = active.clone();
        cycle.push(module.name.clone());
        return Err(Error::runtime(format!(
            "circular module dependency: {}",
            cycle.join(" -> ")
        )));
    }
    active.push(module.name.clone());
    let mut context = host.module_child().with_fuel(*fuel);
    for (bindings, specifier) in &module.imports {
        let source = loader.load(specifier, &module.name)?;
        let dependency = Engine::new().compile_module(source.name(), source.source())?;
        let exports = execute_module(host, &dependency, loader, cache, active, fuel, stats)?;
        for (public, local) in bindings {
            let Some(value) = exports.get(public) else {
                return Err(Error::runtime(format!(
                    "module {} does not export {public}",
                    dependency.name
                )));
            };
            context.set_global(local, value.clone());
        }
    }
    let result = context.run_program(&module.program);
    let execution = context.last_execution();
    *fuel = execution.fuel_remaining;
    stats.instructions += execution.instructions;
    stats.call_depth_peak = stats.call_depth_peak.max(execution.call_depth_peak);
    stats.name_loads += execution.name_loads;
    stats.name_stores += execution.name_stores;
    stats.calls += execution.calls;
    stats.container_ops += execution.container_ops;
    stats.iterator_ops += execution.iterator_ops;
    stats.exception_ops += execution.exception_ops;
    stats.value_allocations += execution.value_allocations;
    stats.environment_allocations += execution.environment_allocations;
    result?;
    let mut exports = BTreeMap::new();
    for (public, local) in &module.exports {
        let Some(value) = context.get_local(local) else {
            return Err(Error::runtime(format!(
                "module {} exports unbound name {local}",
                module.name
            )));
        };
        exports.insert(public.clone(), value);
    }
    let exports = ModuleExports(exports);
    active.pop();
    cache.insert(module.name.clone(), exports.clone());
    Ok(exports)
}
