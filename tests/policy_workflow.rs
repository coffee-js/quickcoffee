#[path = "../examples/policy_package/support.rs"]
mod support;

use quickcoffee::{
    CancellationToken, ErrorKind, LiveMemoryObservation, LiveMemoryOutcome, ResourceLimit,
    ResourceLimits, Runtime, Value,
};
use std::{path::PathBuf, process::Command};
use support::{AUDIT_KEY, PolicyHost, RequestState, prepare, request};

fn decision_field<'a>(decision: &'a Value, field: &str) -> &'a Value {
    &decision.as_map().expect("decision Map")[field]
}

#[test]
fn policy_package_reuses_artifacts_and_isolates_script_and_host_state() {
    let host = PolicyHost::new().unwrap();
    let mut approved = host.context(
        request("120", "customer-low", "CN", "equipment"),
        RequestState::new("customer-low", "low"),
        true,
        None,
    );
    approved.set_live_memory_observation(LiveMemoryObservation::Checkpointed);
    let retained_before = approved.retained_memory();

    let first = host.run(&mut approved).unwrap();
    assert_eq!(decision_field(&first, "approved").as_bool(), Some(true));
    assert_eq!(
        decision_field(&first, "code").as_str(),
        Some("policy.approved")
    );
    assert_eq!(decision_field(&first, "evaluation").as_number(), Some(1.));
    assert!(approved.get_global("result").is_none());
    assert!(approved.get_global("purchase-policy-audit").is_none());
    assert_eq!(
        approved.host_state::<RequestState>().unwrap().lookups.get(),
        1
    );
    assert_eq!(
        approved.capability(AUDIT_KEY).unwrap().borrow().as_slice(),
        ["policy.approved"]
    );
    let live = approved.last_live_memory_report().unwrap();
    assert_eq!(live.outcome, LiveMemoryOutcome::Success);
    assert!(live.high_water.objects > 0);
    assert!(live.high_water.bytes > 0);
    assert_eq!(approved.retained_memory(), retained_before);

    let second = host.run(&mut approved).unwrap();
    assert_eq!(decision_field(&second, "evaluation").as_number(), Some(1.));
    assert_eq!(
        approved.host_state::<RequestState>().unwrap().lookups.get(),
        2
    );
    assert_eq!(approved.capability(AUDIT_KEY).unwrap().borrow().len(), 2);
    assert_eq!(approved.retained_memory(), retained_before);

    let mut denied = host.context(
        request("120", "customer-high", "CN", "equipment"),
        RequestState::new("customer-high", "high"),
        true,
        None,
    );
    let denied_decision = host.run(&mut denied).unwrap();
    assert_eq!(
        decision_field(&denied_decision, "approved").as_bool(),
        Some(false)
    );
    assert_eq!(
        decision_field(&denied_decision, "code").as_str(),
        Some("policy.risk_denied")
    );
    assert_eq!(denied.capability(AUDIT_KEY).unwrap().borrow().len(), 1);
    assert_eq!(approved.capability(AUDIT_KEY).unwrap().borrow().len(), 2);
}

#[test]
fn policy_package_enforces_capabilities_host_errors_and_cancellation() {
    let host = PolicyHost::new().unwrap();

    let mut denied = host.context(
        request("120", "customer", "CN", "equipment"),
        RequestState::new("customer", "low"),
        false,
        None,
    );
    let error = host.run(&mut denied).unwrap_err();
    assert_eq!(
        error.script_error().unwrap().code(),
        "host.capability_denied"
    );
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("host.coffee")
    );

    let mut unavailable = host.context(
        request("120", "customer", "CN", "equipment"),
        RequestState::new("customer", "unavailable"),
        true,
        None,
    );
    let error = host.run(&mut unavailable).unwrap_err();
    let script = error.script_error().unwrap();
    assert_eq!(script.code(), "host.risk_unavailable");
    assert_eq!(
        script.data().as_map().unwrap()["customer_id"].as_str(),
        Some("customer")
    );

    let cancellation = CancellationToken::new();
    let mut cancelled = host.context(
        request("120", "customer", "CN", "equipment"),
        RequestState::new("customer", "cancel"),
        true,
        Some(cancellation.clone()),
    );
    let error = host.run(&mut cancelled).unwrap_err();
    assert!(cancellation.is_cancelled());
    assert_eq!(error.kind(), ErrorKind::Resource);
    assert_eq!(error.resource_limit(), Some(ResourceLimit::Cancellation));
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("host.coffee")
    );
}

#[test]
fn policy_package_reports_domain_compile_and_execution_resource_boundaries() {
    let host = PolicyHost::new().unwrap();
    let mut invalid_request = request("120", "customer", "CN", "equipment")
        .as_map()
        .unwrap()
        .clone();
    invalid_request.insert("amount".to_owned(), Value::from(120_f64));
    let mut invalid = host.context(
        Value::map(invalid_request),
        RequestState::new("customer", "low"),
        true,
        None,
    );
    let error = host.run(&mut invalid).unwrap_err();
    let script = error.script_error().unwrap();
    assert_eq!(script.code(), "policy.invalid_request");
    assert_eq!(
        script.data().as_map().unwrap()["field"].as_str(),
        Some("amount")
    );
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("core.litcoffee")
    );
    assert!(error.labels()[0].span.start.line > 1);

    let mut low_fuel = host.context(
        request("120", "customer", "CN", "equipment"),
        RequestState::new("customer", "low"),
        true,
        None,
    );
    low_fuel.set_fuel(1);
    let error = host.run(&mut low_fuel).unwrap_err();
    assert_eq!(error.resource_limit(), Some(ResourceLimit::Fuel));

    let mut depth = host.context(
        request("120", "customer", "CN", "equipment"),
        RequestState::new("customer", "low"),
        true,
        None,
    );
    depth.set_max_call_depth(0);
    let error = host.run(&mut depth).unwrap_err();
    assert_eq!(error.resource_limit(), Some(ResourceLimit::CallDepth));

    let transient_limits = host
        .runtime
        .execution_policy()
        .resource_limits()
        .with_max_transient_managed_objects(1);
    let mut transient = host.context(
        request("120", "customer", "CN", "equipment"),
        RequestState::new("customer", "low"),
        true,
        None,
    );
    transient.set_resource_limits(transient_limits);
    let error = host.run(&mut transient).unwrap_err();
    assert_eq!(
        error.resource_limit(),
        Some(ResourceLimit::TransientManagedObjects)
    );

    let retained_limits = ResourceLimits::default()
        .with_max_retained_managed_objects(1)
        .with_max_transient_managed_objects(20_000)
        .with_max_transient_managed_bytes(2_000_000);
    let mut retained = host.context(
        request("120", "customer", "CN", "equipment"),
        RequestState::new("customer", "low"),
        true,
        None,
    );
    retained.set_resource_limits(retained_limits);
    let error = host.run(&mut retained).unwrap_err();
    assert_eq!(
        error.resource_limit(),
        Some(ResourceLimit::RetainedManagedObjects)
    );
    assert_eq!(retained.last_execution().instructions, 0);

    let runtime = Runtime::builder()
        .execution_policy(
            host.runtime.execution_policy().with_compile_limits(
                host.runtime
                    .compile_limits()
                    .with_max_module_graph_modules(2),
            ),
        )
        .build();
    let error = prepare(&runtime).unwrap_err();
    assert_eq!(
        error.resource_limit(),
        Some(ResourceLimit::ModuleGraphModules)
    );
}

#[test]
fn policy_core_is_qtestable_without_granting_cli_host_authority() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/policy_package");
    let root_text = root.to_str().unwrap();

    let fingerprint = Command::new(env!("CARGO_BIN_EXE_qcoffee"))
        .args(["--fingerprint", "--module-root", root_text, "test"])
        .output()
        .unwrap();
    assert!(
        fingerprint.status.success(),
        "{}",
        String::from_utf8_lossy(&fingerprint.stderr)
    );
    let fingerprint = String::from_utf8(fingerprint.stdout).unwrap();
    assert_eq!(fingerprint.trim().len(), 16);

    let tested = Command::new(env!("CARGO_BIN_EXE_qtest"))
        .args(["--module-root", root_text, "test"])
        .output()
        .unwrap();
    assert!(
        tested.status.success(),
        "{}",
        String::from_utf8_lossy(&tested.stderr)
    );
    assert_eq!(
        String::from_utf8(tested.stdout).unwrap(),
        "ok test.coffee\n"
    );

    let ambient = Command::new(env!("CARGO_BIN_EXE_qcoffee"))
        .args(["--module-root", root_text, "host"])
        .output()
        .unwrap();
    assert!(!ambient.status.success());
    assert!(String::from_utf8_lossy(&ambient.stderr).contains("unknown name 'host_risk'"));
}
