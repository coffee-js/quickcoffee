//! Run with `cargo bench --bench policy_package`.
//!
//! Measures the real multi-file purchase policy with contextual host callbacks,
//! typed state, and an allowlisted audit sink. The per-worker section is deliberately
//! sequential: it records the documented one-Runtime-per-worker ownership baseline
//! without claiming that Runtime or Context is Send/Sync.
#[path = "../examples/policy_package/support.rs"]
mod support;

use quickcoffee::Value;
use std::{hint::black_box, time::Instant};
use support::{ExecutionPolicy, PolicyHost, RequestState, prepare, request, runtime};

fn short_lived(host: &PolicyHost, count: usize) {
    let policy = ExecutionPolicy::bounded();
    let request = request("120", "benchmark", "CN", "equipment");
    let mut instructions = 0_u64;
    let mut managed_objects = 0_u64;
    let mut managed_bytes = 0_u64;
    let start = Instant::now();
    for _ in 0..count {
        let mut context = host.context(
            request.clone(),
            RequestState::new("benchmark", "low"),
            true,
            None,
            policy,
        );
        black_box(host.run(&mut context).expect("short policy request"));
        let stats = context.last_execution();
        instructions += stats.instructions;
        managed_objects += stats.managed_objects_allocated;
        managed_bytes += stats.managed_bytes_allocated;
    }
    let elapsed = start.elapsed();
    println!(
        "policy-short-{count}: {:.3}ms total, {:.0} requests/s, {:.1}us/request, {:.0} instructions/request, {:.0} managed-objects/request, {:.0} managed-bytes/request",
        elapsed.as_secs_f64() * 1_000.,
        count as f64 / elapsed.as_secs_f64(),
        elapsed.as_secs_f64() * 1_000_000. / count as f64,
        instructions as f64 / count as f64,
        managed_objects as f64 / count as f64,
        managed_bytes as f64 / count as f64,
    );
}

fn long_lived(host: &PolicyHost, count: usize) {
    let mut context = host.context(
        request("120", "benchmark", "CN", "equipment"),
        RequestState::new("benchmark", "low"),
        true,
        None,
        ExecutionPolicy::bounded(),
    );
    let mut instructions = 0_u64;
    let mut managed_objects = 0_u64;
    let mut managed_bytes = 0_u64;
    let start = Instant::now();
    for _ in 0..count {
        let result = host.run(&mut context).expect("long-lived policy request");
        assert_eq!(
            result
                .as_map()
                .and_then(|decision| decision.get("evaluation"))
                .and_then(Value::as_number),
            Some(1.)
        );
        black_box(result);
        let stats = context.last_execution();
        instructions += stats.instructions;
        managed_objects += stats.managed_objects_allocated;
        managed_bytes += stats.managed_bytes_allocated;
    }
    let elapsed = start.elapsed();
    let retained = context.sample_retained_memory();
    println!(
        "policy-long-{count}: {:.3}ms total, {:.0} requests/s, {:.1}us/request, {:.0} instructions/request, {:.0} managed-objects/request, {:.0} managed-bytes/request, retained={} objects/{} bytes",
        elapsed.as_secs_f64() * 1_000.,
        count as f64 / elapsed.as_secs_f64(),
        elapsed.as_secs_f64() * 1_000_000. / count as f64,
        instructions as f64 / count as f64,
        managed_objects as f64 / count as f64,
        managed_bytes as f64 / count as f64,
        retained.objects,
        retained.bytes,
    );
}

fn context_cost(host: &PolicyHost) {
    let iterations = 10_000;
    let policy = ExecutionPolicy::bounded();
    let request = request("120", "benchmark", "CN", "equipment");
    let start = Instant::now();
    for _ in 0..iterations {
        black_box(host.context(
            request.clone(),
            RequestState::new("benchmark", "low"),
            true,
            None,
            policy,
        ));
    }
    let elapsed = start.elapsed();
    println!(
        "policy-context-{iterations}: {:.3}ms total, {:.1}ns/context",
        elapsed.as_secs_f64() * 1_000.,
        elapsed.as_secs_f64() * 1_000_000_000. / iterations as f64,
    );
}

fn observed_memory(host: &PolicyHost) {
    let mut context = host.context(
        request("120", "observed", "CN", "equipment"),
        RequestState::new("observed", "low"),
        true,
        None,
        ExecutionPolicy::bounded().observed(),
    );
    black_box(host.run(&mut context).expect("observed policy request"));
    let report = context
        .last_live_memory_report()
        .expect("live observation enabled");
    println!(
        "policy-live: samples={}, final={} objects/{} bytes, high-water={} objects/{} bytes",
        report.samples,
        report.final_snapshot.objects,
        report.final_snapshot.bytes,
        report.high_water.objects,
        report.high_water.bytes,
    );
}

fn per_worker_baseline() {
    let worker_count = 4;
    let start = Instant::now();
    let workers = (0..worker_count)
        .map(|_| PolicyHost::new().expect("worker policy prepares"))
        .collect::<Vec<_>>();
    let setup = start.elapsed();

    let count = 1_000;
    let policy = ExecutionPolicy::bounded();
    let request = request("120", "worker", "CN", "equipment");
    let start = Instant::now();
    for index in 0..count {
        let worker = &workers[index % workers.len()];
        let mut context = worker.context(
            request.clone(),
            RequestState::new("worker", "low"),
            true,
            None,
            policy,
        );
        black_box(worker.run(&mut context).expect("worker policy request"));
    }
    let elapsed = start.elapsed();
    println!(
        "policy-workers-{worker_count}: {:.3}ms setup, {:.3}ms for {count} sequentially distributed requests, {:.0} requests/s",
        setup.as_secs_f64() * 1_000.,
        elapsed.as_secs_f64() * 1_000.,
        count as f64 / elapsed.as_secs_f64(),
    );
}

fn main() {
    let start = Instant::now();
    for _ in 0..10 {
        let runtime = runtime();
        black_box(prepare(&runtime).expect("cold policy package preflights"));
    }
    let elapsed = start.elapsed();
    println!(
        "policy-cold-prepare-10: {:.3}ms total, {:.1}us/package",
        elapsed.as_secs_f64() * 1_000.,
        elapsed.as_secs_f64() * 100_000.,
    );

    let cached_runtime = runtime();
    let start = Instant::now();
    for _ in 0..100 {
        black_box(prepare(&cached_runtime).expect("cached policy package preflights"));
    }
    let elapsed = start.elapsed();
    println!(
        "policy-cached-prepare-100: {:.3}ms total, {:.1}us/package",
        elapsed.as_secs_f64() * 1_000.,
        elapsed.as_secs_f64() * 10_000.,
    );

    let host = PolicyHost::new().expect("policy host prepares");
    context_cost(&host);
    observed_memory(&host);
    for count in [10, 100, 1_000] {
        short_lived(&host, count);
        long_lived(&host, count);
    }
    per_worker_baseline();
}
