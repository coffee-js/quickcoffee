#[path = "policy_package/support.rs"]
mod support;

use quickcoffee::CancellationToken;
use std::{thread, time::Instant};
use support::{PolicyHost, RequestState, request};

const WORKERS: usize = 4;
const REQUESTS_PER_WORKER: usize = 8;

#[derive(Debug)]
struct WorkerSummary {
    worker: usize,
    requests: usize,
    setup_micros: u128,
    package_modules: usize,
    module_cache_entries: usize,
    last_decision_code: String,
}

fn run_worker(worker: usize, cancellation: CancellationToken) -> Result<WorkerSummary, String> {
    // Runtime, ModulePackage, Context, Value, and Error all stay inside this
    // worker. Only the Send + Sync cancellation signal entered from the host.
    let setup = Instant::now();
    let host = PolicyHost::new().map_err(|error| error.to_string())?;
    let setup_micros = setup.elapsed().as_micros();
    let customer_id = format!("worker-{worker}");
    let mut last_decision_code = String::new();

    for _ in 0..REQUESTS_PER_WORKER {
        let mut context = host.context(
            request("120", &customer_id, "CN", "equipment"),
            RequestState::new(&customer_id, "low"),
            true,
            Some(cancellation.clone()),
        );
        let result = host.run(&mut context).map_err(|error| error.to_string())?;
        last_decision_code = result
            .as_map()
            .and_then(|decision| decision.get("code"))
            .and_then(|code| code.as_str())
            .ok_or_else(|| "policy result has no String code".to_owned())?
            .to_owned();
        // `result` and `context` are dropped here rather than crossing the
        // worker boundary. The summary below contains ordinary Rust data.
    }

    let cache = host.runtime.cache_stats();
    Ok(WorkerSummary {
        worker,
        requests: REQUESTS_PER_WORKER,
        setup_micros,
        package_modules: host.package.module_count(),
        module_cache_entries: cache.module_entries,
        last_decision_code,
    })
}

fn main() {
    let mut controls = Vec::with_capacity(WORKERS);
    let mut handles = Vec::with_capacity(WORKERS);

    for worker in 0..WORKERS {
        let control = CancellationToken::new();
        let worker_cancellation = control.clone();
        controls.push(control);
        handles.push(thread::spawn(move || {
            run_worker(worker, worker_cancellation)
        }));
    }

    for handle in handles {
        let summary = handle
            .join()
            .expect("policy worker must not panic")
            .expect("policy worker must complete");
        assert_eq!(summary.last_decision_code, "policy.approved");
        println!(
            "worker {}: {} requests, {}us setup, {} package modules, {} module-cache entries, last={}",
            summary.worker,
            summary.requests,
            summary.setup_micros,
            summary.package_modules,
            summary.module_cache_entries,
            summary.last_decision_code,
        );
    }

    assert!(controls.iter().all(|control| !control.is_cancelled()));
}
