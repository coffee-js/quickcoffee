use crate::{
    CompileLimits, Context, Engine, Error, ExecutionStats, Program, ResourceLimit, Runtime, Value,
    bytecode::FingerprintEncoder, lowering, parser,
};
use cap_std::{ambient_authority, fs::Dir};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Read},
    path::{Component, Path},
    sync::Arc,
};

/// Canonical encoding version used by [`Engine::fingerprint_module_graph`].
///
/// A semantic change to the graph encoding must increment this value. The
/// existing bytecode fingerprints remain a separate compatibility domain.
pub const MODULE_GRAPH_FINGERPRINT_VERSION: u64 = 1;

const MODULE_GRAPH_FINGERPRINT_DOMAIN: &str = "quickcoffee.module-graph";
const MODULE_SOURCE_FINGERPRINT_DOMAIN: &str = "quickcoffee.module-source";

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

/// An opt-in filesystem loader confined to one open directory capability.
///
/// Imports must use explicit `./` or `../` specifiers. Module names are
/// root-relative UTF-8 paths with `/` separators and a `.coffee` or
/// `.litcoffee` extension. Both
/// lexical traversal above the root and symlink targets outside it are
/// rejected before source is returned to the engine.
#[derive(Clone, Debug)]
pub struct RestrictedFileModuleLoader {
    root: Arc<Dir>,
    max_source_bytes: usize,
}
impl RestrictedFileModuleLoader {
    /// Creates a loader rooted at an existing directory.
    pub fn new(root: impl AsRef<Path>) -> Result<Self, Error> {
        let requested_root = root.as_ref();
        let root = Dir::open_ambient_dir(requested_root, ambient_authority()).map_err(|_| {
            Error::runtime(format!(
                "module root is unavailable: {}",
                requested_root.display()
            ))
        })?;
        Ok(Self {
            root: Arc::new(root),
            max_source_bytes: CompileLimits::default().max_source_bytes(),
        })
    }

    /// Returns this loader with a replacement per-file source byte boundary.
    ///
    /// Match this value to the [`CompileLimits`] used by the consuming Engine
    /// or Runtime when raising the default boundary.
    pub fn with_max_source_bytes(mut self, limit: usize) -> Self {
        self.max_source_bytes = limit;
        self
    }

    /// Loads one root-relative entry module, inferring `.coffee` when omitted.
    pub fn load_entry(&self, name: &str) -> Result<ModuleSource, Error> {
        let name = entry_name(name)?;
        self.load_name(&name, name.as_str())
    }

    fn load_name(&self, name: &str, requested: &str) -> Result<ModuleSource, Error> {
        let canonical = self.root.canonicalize(name).map_err(|error| {
            if error.kind() == io::ErrorKind::PermissionDenied {
                Error::runtime(format!("module path escapes configured root: {requested}"))
            } else {
                Error::runtime(format!("module not found: {name}"))
            }
        })?;
        let mut parts = Vec::new();
        for component in canonical.components() {
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
            if component.contains(['\\', ':']) {
                return Err(Error::runtime(format!(
                    "invalid module target: {requested}"
                )));
            }
            parts.push(component);
        }
        let canonical_name = parts.join("/");
        if !canonical
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(is_module_extension)
        {
            return Err(Error::runtime(format!(
                "invalid module target: {requested}"
            )));
        }
        let mut file = self.root.open(&canonical).map_err(|_| {
            Error::runtime(format!("module source is not readable: {canonical_name}"))
        })?;
        let metadata = file.metadata().map_err(|_| {
            Error::runtime(format!("module source is not readable: {canonical_name}"))
        })?;
        if !metadata.is_file() {
            return Err(Error::runtime(format!(
                "invalid module target: {requested}"
            )));
        }
        if metadata.len() > self.max_source_bytes as u64 {
            return Err(source_limit_error(&canonical_name, self.max_source_bytes));
        }
        let mut source = String::new();
        file.by_ref()
            .take((self.max_source_bytes as u64).saturating_add(1))
            .read_to_string(&mut source)
            .map_err(|error| {
                if error.kind() == io::ErrorKind::InvalidData {
                    Error::runtime(format!(
                        "module source is not readable UTF-8: {canonical_name}"
                    ))
                } else {
                    Error::runtime(format!("module source is not readable: {canonical_name}"))
                }
            })?;
        if source.len() > self.max_source_bytes {
            return Err(source_limit_error(&canonical_name, self.max_source_bytes));
        }
        Ok(ModuleSource::new(canonical_name, source))
    }
}

fn source_limit_error(source_name: &str, limit: usize) -> Error {
    Error::resource(
        ResourceLimit::SourceBytes,
        format!("source exceeds configured UTF-8 byte limit of {limit}"),
    )
    .at_line(1)
    .with_source_name(source_name)
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
        || parts.last().is_none_or(|part| {
            part.rsplit_once('.')
                .is_none_or(|(_, extension)| !is_module_extension(extension))
        })
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
        if !is_module_extension(extension) {
            return Err(Error::runtime(format!(
                "invalid module {subject}: {requested}"
            )));
        }
    } else {
        name.push_str(".coffee");
    }
    Ok(())
}

fn is_module_extension(extension: &str) -> bool {
    matches!(extension, "coffee" | "litcoffee")
}

/// Compiled static imports and named exports for one QuickCoffee module.
#[derive(Clone, Debug)]
pub struct Module {
    name: String,
    program: Program,
    source_fingerprint: u64,
    source_bytes: usize,
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
    /// Returns the raw UTF-8 source bytes charged to static graph limits.
    pub fn source_bytes(&self) -> usize {
        self.source_bytes
    }
    /// Returns recursively reachable bytecode instructions in this module.
    pub fn instruction_count(&self) -> usize {
        self.program.instruction_count()
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
        self.check_source_limit(Some(name.as_str()), source)?;
        let prepared = crate::source::prepare(Some(name.as_str()), source)
            .map_err(|error| error.with_source_name(name.as_str()))?;
        let syntax =
            parser::parse_module_with_columns(&prepared.text, prepared.columns_are_precise)
                .map_err(|error| error.with_source_name(name.as_str()))?;
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
        let instruction_count = source_map.instruction_count;
        self.check_bytecode_limit(Some(name.as_str()), instruction_count)?;
        lowering::verify_mapped(&chunk, &source_map)
            .map_err(|error| error.with_source_name(name.as_str()))?;
        let program =
            Program::from_compiled(chunk, source_map, Some(name.as_str()), instruction_count);
        Ok(Module {
            name,
            program,
            source_fingerprint: fingerprint_module_source(source),
            source_bytes: source.len(),
            imports: syntax.imports,
            exports: syntax
                .exports
                .into_iter()
                .map(|(public, local, _)| (public, local))
                .collect(),
        })
    }

    /// Loads and verifies the complete static dependency graph without
    /// executing it, then returns its deterministic versioned fingerprint.
    ///
    /// The fingerprint covers canonical module names, raw UTF-8 sources,
    /// verified program fingerprints, named imports and exports, and resolved
    /// dependency edges. Missing modules, invalid dependencies, and cycles are
    /// returned as errors and never produce a partial fingerprint.
    pub fn fingerprint_module_graph(
        &self,
        entry: &Module,
        loader: &dyn ModuleLoader,
    ) -> Result<u64, Error> {
        let mut identities = BTreeMap::new();
        let mut graph = BTreeMap::new();
        let mut active = Vec::new();
        let mut budget = ModuleGraphBudget::new(self.compile_limits());
        collect_module_graph(
            self,
            entry,
            loader,
            &mut identities,
            &mut graph,
            &mut active,
            &mut budget,
        )?;
        Ok(encode_module_graph(entry.name(), &graph))
    }
}

impl Runtime {
    /// Compiles one named module, reusing an exact verified cache entry.
    ///
    /// The complete canonical name and raw UTF-8 source form the cache
    /// identity. Only compilation artifacts are shared; module exports and
    /// evaluation state always belong to an individual Context run.
    pub fn compile_module(&self, name: impl Into<String>, source: &str) -> Result<Module, Error> {
        let name = name.into();
        self.engine()
            .check_source_limit(Some(name.as_str()), source)?;
        if let Some(module) = self.cached_module(&name, source) {
            return Ok(module);
        }
        let module = self.engine().compile_module(name, source)?;
        self.cache_module(module.clone(), source);
        Ok(module)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ModuleIdentity {
    source_fingerprint: u64,
    program_fingerprint: u64,
    imports: Vec<(Vec<(String, String)>, String)>,
    exports: Vec<(String, String)>,
}

struct ModuleGraphBudget {
    limits: CompileLimits,
    sources: BTreeMap<String, u64>,
    source_bytes: usize,
}
impl ModuleGraphBudget {
    fn new(limits: CompileLimits) -> Self {
        Self {
            limits,
            sources: BTreeMap::new(),
            source_bytes: 0,
        }
    }

    fn reserve_module(&mut self, module: &Module) -> Result<(), Error> {
        self.reserve(
            module.name(),
            module.source_fingerprint,
            module.source_bytes,
        )
    }

    fn reserve_source(&mut self, source: &ModuleSource) -> Result<(), Error> {
        if source.source().len() > self.limits.max_source_bytes() {
            return Err(source_limit_error(
                source.name(),
                self.limits.max_source_bytes(),
            ));
        }
        self.reserve(
            source.name(),
            fingerprint_module_source(source.source()),
            source.source().len(),
        )
    }

    fn reserve(&mut self, name: &str, fingerprint: u64, bytes: usize) -> Result<(), Error> {
        if let Some(existing) = self.sources.get(name) {
            if *existing != fingerprint {
                return Err(Error::runtime(format!(
                    "module canonical name resolved to inconsistent source: {name}"
                )));
            }
            return Ok(());
        }
        if self.sources.len() >= self.limits.max_module_graph_modules() {
            return Err(Error::resource(
                ResourceLimit::ModuleGraphModules,
                format!(
                    "module graph exceeds configured unique module limit of {}",
                    self.limits.max_module_graph_modules()
                ),
            ));
        }
        let source_bytes = self.source_bytes.checked_add(bytes).ok_or_else(|| {
            Error::resource(
                ResourceLimit::ModuleGraphSourceBytes,
                "module graph source byte count overflowed",
            )
        })?;
        if source_bytes > self.limits.max_module_graph_source_bytes() {
            return Err(Error::resource(
                ResourceLimit::ModuleGraphSourceBytes,
                format!(
                    "module graph exceeds configured cumulative source byte limit of {}",
                    self.limits.max_module_graph_source_bytes()
                ),
            ));
        }
        self.sources.insert(name.to_owned(), fingerprint);
        self.source_bytes = source_bytes;
        Ok(())
    }
}

impl From<&Module> for ModuleIdentity {
    fn from(module: &Module) -> Self {
        Self {
            source_fingerprint: module.source_fingerprint,
            program_fingerprint: module.program.fingerprint(),
            imports: module.imports.clone(),
            exports: module.exports.clone(),
        }
    }
}

#[derive(Clone, Debug)]
struct GraphImport {
    bindings: Vec<(String, String)>,
    specifier: String,
    dependency: String,
}

#[derive(Clone, Debug)]
struct GraphModule {
    source_fingerprint: u64,
    program_fingerprint: u64,
    imports: Vec<GraphImport>,
    exports: Vec<(String, String)>,
}

fn fingerprint_module_source(source: &str) -> u64 {
    let mut encoder = FingerprintEncoder::new();
    encoder.string(MODULE_SOURCE_FINGERPRINT_DOMAIN);
    encoder.u64(MODULE_GRAPH_FINGERPRINT_VERSION);
    encoder.string(source);
    encoder.finish()
}

fn collect_module_graph(
    engine: &Engine,
    module: &Module,
    loader: &dyn ModuleLoader,
    identities: &mut BTreeMap<String, ModuleIdentity>,
    graph: &mut BTreeMap<String, GraphModule>,
    active: &mut Vec<String>,
    budget: &mut ModuleGraphBudget,
) -> Result<(), Error> {
    budget.reserve_module(module)?;
    if active.iter().any(|name| name == module.name()) {
        let mut cycle = active.clone();
        cycle.push(module.name.clone());
        return Err(Error::runtime(format!(
            "circular module dependency: {}",
            cycle.join(" -> ")
        )));
    }

    let identity = ModuleIdentity::from(module);
    if let Some(existing) = identities.get(module.name()) {
        if existing != &identity {
            return Err(Error::runtime(format!(
                "module canonical name resolved to inconsistent source: {}",
                module.name()
            )));
        }
        return Ok(());
    }
    identities.insert(module.name.clone(), identity);
    active.push(module.name.clone());

    let mut imports = Vec::with_capacity(module.imports.len());
    for (bindings, specifier) in &module.imports {
        let source = loader.load(specifier, module.name())?;
        budget.reserve_source(&source)?;
        let dependency = engine.compile_module(source.name(), source.source())?;
        let dependency_name = dependency.name.clone();
        collect_module_graph(
            engine,
            &dependency,
            loader,
            identities,
            graph,
            active,
            budget,
        )?;
        for (public, _) in bindings {
            if !dependency
                .exports
                .iter()
                .any(|(exported, _)| exported == public)
            {
                return Err(Error::runtime(format!(
                    "module {} does not export {public}",
                    dependency.name
                )));
            }
        }
        imports.push(GraphImport {
            bindings: bindings.clone(),
            specifier: specifier.clone(),
            dependency: dependency_name,
        });
    }

    let mut exports = module.exports.clone();
    exports.sort();
    active.pop();
    graph.insert(
        module.name.clone(),
        GraphModule {
            source_fingerprint: module.source_fingerprint,
            program_fingerprint: module.program.fingerprint(),
            imports,
            exports,
        },
    );
    Ok(())
}

fn encode_module_graph(entry_name: &str, graph: &BTreeMap<String, GraphModule>) -> u64 {
    let mut encoder = FingerprintEncoder::new();
    encoder.string(MODULE_GRAPH_FINGERPRINT_DOMAIN);
    encoder.u64(MODULE_GRAPH_FINGERPRINT_VERSION);
    encoder.string(entry_name);
    encoder.u64(graph.len() as u64);
    for (name, module) in graph {
        encoder.string(name);
        encoder.u64(module.source_fingerprint);
        encoder.u64(module.program_fingerprint);
        encoder.u64(module.imports.len() as u64);
        for import in &module.imports {
            encoder.string(&import.specifier);
            encoder.string(&import.dependency);
            encoder.u64(import.bindings.len() as u64);
            for (public, local) in &import.bindings {
                encoder.string(public);
                encoder.string(local);
            }
        }
        encoder.u64(module.exports.len() as u64);
        for (public, local) in &module.exports {
            encoder.string(public);
            encoder.string(local);
        }
    }
    encoder.finish()
}

impl Context {
    /// Loads dependencies through `loader`, runs `module` in a private module
    /// environment, and returns only its named exports.
    pub fn run_module(
        &mut self,
        module: &Module,
        loader: &dyn ModuleLoader,
    ) -> Result<ModuleExports, Error> {
        let mut identities = BTreeMap::new();
        let mut prepared = BTreeMap::new();
        let mut prepare_active = Vec::new();
        let mut budget = ModuleGraphBudget::new(self.compile_limits());
        prepare_module_graph(
            self,
            module,
            loader,
            &mut identities,
            &mut prepared,
            &mut prepare_active,
            &mut budget,
        )?;
        let mut cache = BTreeMap::new();
        let mut active = Vec::new();
        let mut fuel = self.fuel();
        let mut stats = ExecutionStats::default();
        let result = execute_prepared_module(
            self,
            module.name(),
            &prepared,
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

#[derive(Clone)]
struct PreparedModule {
    module: Module,
    dependencies: Vec<String>,
}

fn prepare_module_graph(
    host: &Context,
    module: &Module,
    loader: &dyn ModuleLoader,
    identities: &mut BTreeMap<String, ModuleIdentity>,
    prepared: &mut BTreeMap<String, PreparedModule>,
    active: &mut Vec<String>,
    budget: &mut ModuleGraphBudget,
) -> Result<(), Error> {
    budget.reserve_module(module)?;
    if active.iter().any(|name| name == module.name()) {
        let mut cycle = active.clone();
        cycle.push(module.name.clone());
        return Err(Error::runtime(format!(
            "circular module dependency: {}",
            cycle.join(" -> ")
        )));
    }
    let identity = ModuleIdentity::from(module);
    if let Some(existing) = identities.get(module.name()) {
        if existing != &identity {
            return Err(Error::runtime(format!(
                "module canonical name resolved to inconsistent source: {}",
                module.name()
            )));
        }
        return Ok(());
    }
    identities.insert(module.name.clone(), identity);
    active.push(module.name.clone());
    let mut dependencies = Vec::with_capacity(module.imports.len());
    for (bindings, specifier) in &module.imports {
        let source = loader.load(specifier, module.name())?;
        budget.reserve_source(&source)?;
        let dependency = host.compile_module(source.name(), source.source())?;
        prepare_module_graph(
            host,
            &dependency,
            loader,
            identities,
            prepared,
            active,
            budget,
        )?;
        for (public, _) in bindings {
            if !dependency
                .exports
                .iter()
                .any(|(exported, _)| exported == public)
            {
                return Err(Error::runtime(format!(
                    "module {} does not export {public}",
                    dependency.name
                )));
            }
        }
        dependencies.push(dependency.name.clone());
    }
    active.pop();
    prepared.insert(
        module.name.clone(),
        PreparedModule {
            module: module.clone(),
            dependencies,
        },
    );
    Ok(())
}

fn execute_prepared_module(
    host: &Context,
    module_name: &str,
    prepared: &BTreeMap<String, PreparedModule>,
    cache: &mut BTreeMap<String, ModuleExports>,
    active: &mut Vec<String>,
    fuel: &mut u64,
    stats: &mut ExecutionStats,
) -> Result<ModuleExports, Error> {
    if let Some(exports) = cache.get(module_name) {
        return Ok(exports.clone());
    }
    let prepared_module = prepared
        .get(module_name)
        .expect("prepared module graph contains every resolved edge");
    let module = &prepared_module.module;
    if active.iter().any(|name| name == module_name) {
        let mut cycle = active.clone();
        cycle.push(module.name.clone());
        return Err(Error::runtime(format!(
            "circular module dependency: {}",
            cycle.join(" -> ")
        )));
    }
    active.push(module.name.clone());
    let mut context = host.module_child().with_fuel(*fuel);
    for ((bindings, _), dependency_name) in module.imports.iter().zip(&prepared_module.dependencies)
    {
        let dependency = &prepared
            .get(dependency_name)
            .expect("prepared dependency exists")
            .module;
        let exports =
            execute_prepared_module(host, dependency_name, prepared, cache, active, fuel, stats)?;
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
    let limits = host.resource_limits();
    let remaining_objects = if limits.max_transient_managed_objects() == u64::MAX {
        u64::MAX
    } else {
        limits
            .max_transient_managed_objects()
            .saturating_sub(stats.managed_objects_allocated)
    };
    let remaining_bytes = if limits.max_transient_managed_bytes() == u64::MAX {
        u64::MAX
    } else {
        limits
            .max_transient_managed_bytes()
            .saturating_sub(stats.managed_bytes_allocated)
    };
    context.set_resource_limits(
        limits
            .with_max_transient_managed_objects(remaining_objects)
            .with_max_transient_managed_bytes(remaining_bytes),
    );
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
    stats.managed_objects_allocated = stats
        .managed_objects_allocated
        .saturating_add(execution.managed_objects_allocated);
    stats.managed_bytes_allocated = stats
        .managed_bytes_allocated
        .saturating_add(execution.managed_bytes_allocated);
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
