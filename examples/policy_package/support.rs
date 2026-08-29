use quickcoffee::{
    CancellationToken, CapabilityKey, CapabilityKind, CompileLimits, Context, Error,
    LiveMemoryObservation, ModulePackage, ResourceLimits, RestrictedFileModuleLoader, Runtime,
    Value,
};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
};

pub type AuditLog = RefCell<Vec<String>>;

pub const AUDIT_KEY: CapabilityKey<AuditLog> =
    CapabilityKey::new(CapabilityKind::Logging, "purchase-policy-audit");

pub struct RequestState {
    pub customer_id: String,
    pub risk_band: String,
    pub lookups: Cell<u64>,
}

impl RequestState {
    pub fn new(customer_id: impl Into<String>, risk_band: impl Into<String>) -> Self {
        Self {
            customer_id: customer_id.into(),
            risk_band: risk_band.into(),
            lookups: Cell::new(0),
        }
    }
}

#[derive(Clone, Copy)]
pub struct ExecutionPolicy {
    pub fuel: u64,
    pub max_call_depth: usize,
    pub resource_limits: ResourceLimits,
    pub live_memory_observation: LiveMemoryObservation,
}

impl ExecutionPolicy {
    pub fn bounded() -> Self {
        Self {
            fuel: 250_000,
            max_call_depth: 64,
            resource_limits: ResourceLimits::default()
                .with_max_string_bytes(8_192)
                .with_max_array_items(128)
                .with_max_map_entries(128)
                .with_max_collection_operation_items(256)
                .with_max_text_operation_bytes(16_384)
                .with_max_integer_bits(256)
                .with_max_decimal_coefficient_bits(256)
                .with_max_decimal_scale(8)
                .with_max_retained_managed_objects(2_048)
                .with_max_retained_managed_bytes(256_000)
                .with_max_transient_managed_objects(20_000)
                .with_max_transient_managed_bytes(2_000_000),
            live_memory_observation: LiveMemoryObservation::Off,
        }
    }

    pub fn observed(mut self) -> Self {
        self.live_memory_observation = LiveMemoryObservation::Checkpointed;
        self
    }
}

pub fn compile_limits() -> CompileLimits {
    CompileLimits::default()
        .with_max_source_bytes(128_000)
        .with_max_bytecode_instructions(40_000)
        .with_max_module_graph_modules(8)
        .with_max_module_graph_source_bytes(256_000)
}

pub fn runtime() -> Runtime {
    Runtime::builder()
        .compile_limits(compile_limits())
        .program_cache_entries(16)
        .module_cache_entries(16)
        .build()
}

pub fn loader() -> Result<RestrictedFileModuleLoader, Error> {
    RestrictedFileModuleLoader::new(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/policy_package"),
    )
}

pub fn prepare(runtime: &Runtime) -> Result<ModulePackage, Error> {
    let loader = loader()?;
    let source = loader.load_entry("host")?;
    let entry = runtime.compile_module(source.name(), source.source())?;
    runtime.prepare_module_package(&entry, &loader)
}

pub struct PolicyHost {
    pub runtime: Runtime,
    pub package: ModulePackage,
}

impl PolicyHost {
    pub fn new() -> Result<Self, Error> {
        let runtime = runtime();
        let package = prepare(&runtime)?;
        Ok(Self { runtime, package })
    }

    pub fn context(
        &self,
        request: Value,
        state: RequestState,
        allow_audit: bool,
        cancellation: Option<CancellationToken>,
        policy: ExecutionPolicy,
    ) -> Context {
        let callback_cancellation = cancellation.clone();
        let mut builder = self
            .runtime
            .context_builder()
            .fuel(policy.fuel)
            .max_call_depth(policy.max_call_depth)
            .resource_limits(policy.resource_limits)
            .live_memory_observation(policy.live_memory_observation)
            .host_state(state)
            .global("request", request)
            .contextual_native("host_risk", move |call, args| {
                call.check_cancelled()?;
                call.consume_fuel(5)?;
                let [customer] = args else {
                    return Err(Error::runtime("host_risk expects one customer id"));
                };
                let Some(customer) = customer.as_str() else {
                    return Err(Error::runtime("host_risk expects a String customer id"));
                };
                let state = call
                    .host_state::<RequestState>()
                    .ok_or_else(|| Error::runtime("missing purchase request state"))?;
                if customer != state.customer_id {
                    return Err(Error::domain(
                        "host.request_mismatch",
                        "risk lookup customer does not match host request state",
                        Value::map([
                            ("actual", Value::from(customer)),
                            ("expected", Value::from(state.customer_id.as_str())),
                        ]),
                    ));
                }
                state.lookups.set(state.lookups.get() + 1);
                if state.risk_band == "cancel" {
                    let cancellation = callback_cancellation.as_ref().ok_or_else(|| {
                        Error::runtime("cancel risk state requires a cancellation token")
                    })?;
                    cancellation.cancel();
                    call.check_cancelled()?;
                }
                if state.risk_band == "unavailable" {
                    return Err(Error::domain(
                        "host.risk_unavailable",
                        "risk signal is unavailable",
                        Value::map([("customer_id", Value::from(customer))]),
                    ));
                }
                call.record_managed_allocation(1, state.risk_band.len() as u64);
                Ok(Value::from(state.risk_band.clone()))
            });
        if allow_audit {
            builder = builder.capability(AUDIT_KEY, AuditLog::new(Vec::new()));
        }
        if let Some(cancellation) = cancellation {
            builder = builder.cancellation_token(cancellation);
        }
        builder
            .contextual_native("host_audit", |call, args| {
                call.check_cancelled()?;
                call.consume_fuel(3)?;
                let [code] = args else {
                    return Err(Error::runtime("host_audit expects one decision code"));
                };
                let Some(code) = code.as_str() else {
                    return Err(Error::runtime("host_audit expects a String decision code"));
                };
                let audit = call.capability(AUDIT_KEY).ok_or_else(|| {
                    Error::domain(
                        "host.capability_denied",
                        "purchase audit capability denied",
                        Value::map([("capability", Value::from("logging/purchase-policy-audit"))]),
                    )
                })?;
                audit.borrow_mut().push(code.to_owned());
                call.record_managed_allocation(1, code.len() as u64);
                Ok(Value::Nil)
            })
            .build()
    }

    pub fn run(&self, context: &mut Context) -> Result<Value, Error> {
        context
            .run_module_package(&self.package)?
            .get("result")
            .cloned()
            .ok_or_else(|| Error::runtime("policy host module did not export result"))
    }
}

pub fn request(amount: &str, customer_id: &str, country: &str, purpose: &str) -> Value {
    Value::map([
        (
            "amount",
            Value::from(quickcoffee::Decimal::parse(amount).expect("valid request decimal")),
        ),
        ("country", Value::from(country)),
        ("customer_id", Value::from(customer_id)),
        ("purpose", Value::from(purpose)),
    ])
}
