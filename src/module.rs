use crate::{Context, Engine, Error, ExecutionStats, Program, Value, lowering, parser};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

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
/// The core engine never selects a file-system or network source on its own.
/// The host supplies a loader, which receives the literal module specifier and
/// importing module's canonical name, then returns a canonical source name.
/// Loader implementations therefore define the source authority explicitly.
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

/// An opt-in filesystem loader confined to one canonical directory tree.
///
/// Imports must use explicit `./` or `../` specifiers. Module names are
/// root-relative UTF-8 paths with `/` separators and a `.qc` extension. Both
/// lexical traversal above the root and symlink targets outside it are
/// rejected before source is returned to the engine.
#[derive(Clone, Debug)]
pub struct RestrictedFileModuleLoader {
    root: PathBuf,
}
impl RestrictedFileModuleLoader {
    /// Creates a loader rooted at an existing directory.
    pub fn new(root: impl AsRef<Path>) -> Result<Self, Error> {
        let requested_root = root.as_ref();
        let root = fs::canonicalize(requested_root).map_err(|_| {
            Error::runtime(format!(
                "module root is unavailable: {}",
                requested_root.display()
            ))
        })?;
        if !root.is_dir() {
            return Err(Error::runtime(format!(
                "module root is not a directory: {}",
                root.display()
            )));
        }
        Ok(Self { root })
    }

    /// Returns the canonical directory that bounds all module reads.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Loads one root-relative entry module, inferring `.qc` when omitted.
    pub fn load_entry(&self, name: &str) -> Result<ModuleSource, Error> {
        let name = entry_name(name)?;
        self.load_name(&name, name.as_str())
    }

    fn load_name(&self, name: &str, requested: &str) -> Result<ModuleSource, Error> {
        let mut candidate = self.root.clone();
        for component in name.split('/') {
            candidate.push(component);
        }
        let canonical = fs::canonicalize(candidate)
            .map_err(|_| Error::runtime(format!("module not found: {name}")))?;
        if !canonical.starts_with(&self.root) {
            return Err(Error::runtime(format!(
                "module path escapes configured root: {requested}"
            )));
        }
        if !canonical.is_file()
            || canonical.extension().and_then(|value| value.to_str()) != Some("qc")
        {
            return Err(Error::runtime(format!(
                "invalid module target: {requested}"
            )));
        }
        let relative = canonical
            .strip_prefix(&self.root)
            .expect("canonical module is below the canonical root");
        let mut parts = Vec::new();
        for component in relative.components() {
            let Component::Normal(component) = component else {
                return Err(Error::runtime(format!(
                    "invalid module target: {requested}"
                )));
            };
            let Some(component) = component.to_str() else {
                return Err(Error::runtime(format!(
                    "module path is not UTF-8: {requested}"
                )));
            };
            parts.push(component);
        }
        let canonical_name = parts.join("/");
        let source = fs::read_to_string(&canonical).map_err(|_| {
            Error::runtime(format!(
                "module source is not readable UTF-8: {canonical_name}"
            ))
        })?;
        Ok(ModuleSource::new(canonical_name, source))
    }
}
impl ModuleLoader for RestrictedFileModuleLoader {
    fn load(&self, specifier: &str, referrer: &str) -> Result<ModuleSource, Error> {
        let name = import_name(specifier, referrer)?;
        self.load_name(&name, specifier)
    }
}

fn entry_name(name: &str) -> Result<String, Error> {
    if name.is_empty() || name.starts_with('/') || name.contains('\\') || name.contains(':') {
        return Err(Error::runtime(format!("invalid module entry: {name}")));
    }
    let mut parts = Vec::new();
    for part in name.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return Err(Error::runtime(format!("invalid module entry: {name}")));
        }
        parts.push(part.to_owned());
    }
    add_module_extension(&mut parts, name, "entry")?;
    Ok(parts.join("/"))
}

fn import_name(specifier: &str, referrer: &str) -> Result<String, Error> {
    if !(specifier.starts_with("./") || specifier.starts_with("../"))
        || specifier.contains('\\')
        || specifier.contains(':')
    {
        return Err(Error::runtime(format!(
            "invalid module specifier: {specifier}"
        )));
    }
    let mut parts = canonical_referrer(referrer)?;
    parts.pop();
    let mut final_part_is_name = false;
    for part in specifier.split('/') {
        match part {
            "" => {
                return Err(Error::runtime(format!(
                    "invalid module specifier: {specifier}"
                )));
            }
            "." => final_part_is_name = false,
            ".." => {
                if parts.pop().is_none() {
                    return Err(Error::runtime(format!(
                        "module path escapes configured root: {specifier}"
                    )));
                }
                final_part_is_name = false;
            }
            part => {
                parts.push(part.to_owned());
                final_part_is_name = true;
            }
        }
    }
    if !final_part_is_name {
        return Err(Error::runtime(format!(
            "invalid module specifier: {specifier}"
        )));
    }
    add_module_extension(&mut parts, specifier, "specifier")?;
    Ok(parts.join("/"))
}

fn canonical_referrer(referrer: &str) -> Result<Vec<String>, Error> {
    if referrer.is_empty()
        || referrer.starts_with('/')
        || referrer.contains('\\')
        || referrer.contains(':')
    {
        return Err(Error::runtime(format!(
            "invalid module referrer: {referrer}"
        )));
    }
    let parts = referrer.split('/').map(str::to_owned).collect::<Vec<_>>();
    if parts
        .iter()
        .any(|part| part.is_empty() || part == "." || part == "..")
        || parts.last().is_none_or(|part| !part.ends_with(".qc"))
    {
        return Err(Error::runtime(format!(
            "invalid module referrer: {referrer}"
        )));
    }
    Ok(parts)
}

fn add_module_extension(parts: &mut [String], requested: &str, subject: &str) -> Result<(), Error> {
    let name = parts
        .last_mut()
        .expect("validated module paths have a final component");
    if let Some((_, extension)) = name.rsplit_once('.') {
        if extension != "qc" {
            return Err(Error::runtime(format!(
                "invalid module {subject}: {requested}"
            )));
        }
    } else {
        name.push_str(".qc");
    }
    Ok(())
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
        let name = name.into();
        let syntax =
            parser::parse_module(source).map_err(|error| error.with_source_name(name.as_str()))?;
        let mut public_names = BTreeSet::new();
        for (public, _, span) in &syntax.exports {
            if !public_names.insert(public.clone()) {
                return Err(Error::parse(format!("duplicate module export: {public}"))
                    .at_span(span.into_source_span())
                    .with_source_name(name.as_str()));
            }
        }
        let (chunk, source_map) = lowering::compile_mapped(&syntax.body)
            .map_err(|error| error.with_source_name(name.as_str()))?;
        lowering::verify_mapped(&chunk, &source_map)
            .map_err(|error| error.with_source_name(name.as_str()))?;
        let program = Program::from_compiled(chunk, source_map, Some(name.as_str()));
        Ok(Module {
            name,
            program,
            imports: syntax.imports,
            exports: syntax
                .exports
                .into_iter()
                .map(|(public, local, _)| (public, local))
                .collect(),
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
