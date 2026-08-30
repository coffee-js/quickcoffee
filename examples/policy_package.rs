#[path = "policy_package/support.rs"]
mod support;

use quickcoffee::{CancellationToken, LiveMemoryObservation, LiveMemoryOutcome, ResourceLimit};
use support::{AUDIT_KEY, PolicyHost, RequestState, request};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = PolicyHost::new()?;

    for (customer_id, risk_band) in [("customer-low", "low"), ("customer-high", "high")] {
        let mut context = host.context(
            request("120", customer_id, "CN", "equipment"),
            RequestState::new(customer_id, risk_band),
            true,
            None,
        );
        context.set_live_memory_observation(LiveMemoryObservation::Checkpointed);
        let result = host.run(&mut context)?;
        let audits = context.capability(AUDIT_KEY).expect("audit capability");
        let lookups = context
            .host_state::<RequestState>()
            .expect("request state")
            .lookups
            .get();
        let live = context.last_live_memory_report().expect("observed run");
        assert_eq!(live.outcome, LiveMemoryOutcome::Success);
        println!("{customer_id}: {result}");
        eprintln!(
            "risk lookups={lookups} audits={:?} live_high_water={} objects/{} bytes",
            audits.borrow(),
            live.high_water.objects,
            live.high_water.bytes,
        );
    }

    let mut capability_denied = host.context(
        request("120", "customer-denied", "CN", "equipment"),
        RequestState::new("customer-denied", "low"),
        false,
        None,
    );
    let error = host
        .run(&mut capability_denied)
        .expect_err("audit capability must be explicit");
    println!(
        "capability denied: {}",
        error.script_error().expect("domain error").code()
    );

    let cancellation = CancellationToken::new();
    let mut cancelled = host.context(
        request("120", "customer-cancel", "CN", "equipment"),
        RequestState::new("customer-cancel", "cancel"),
        true,
        Some(cancellation),
    );
    let error = host
        .run(&mut cancelled)
        .expect_err("risk callback requests cancellation");
    assert_eq!(error.resource_limit(), Some(ResourceLimit::Cancellation));
    println!("cancelled: {:?}", error.resource_limit());
    Ok(())
}
