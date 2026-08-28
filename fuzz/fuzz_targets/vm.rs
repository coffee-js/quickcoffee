#![no_main]

use libfuzzer_sys::fuzz_target;
use quickcoffee::{Context, ResourceLimits};

const MAX_INPUT_BYTES: usize = 16 * 1024;

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT_BYTES {
        return;
    }
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };
    let limits = ResourceLimits::default()
        .with_max_json_input_bytes(64 * 1024)
        .with_max_json_output_bytes(64 * 1024)
        .with_max_json_string_bytes(32 * 1024)
        .with_max_json_container_items(4 * 1024)
        .with_max_json_values(8 * 1024)
        .with_max_json_nesting_depth(64)
        .with_max_integer_bits(4 * 1024)
        .with_max_decimal_coefficient_bits(4 * 1024)
        .with_max_decimal_scale(1_024)
        .with_max_collection_operation_items(4 * 1024)
        .with_max_text_operation_bytes(64 * 1024)
        .with_max_string_bytes(64 * 1024)
        .with_max_array_items(4 * 1024)
        .with_max_map_entries(4 * 1024)
        .with_max_retained_managed_objects(8 * 1024)
        .with_max_retained_managed_bytes(512 * 1024);
    let mut context = Context::new()
        .with_fuel(50_000)
        .with_max_call_depth(64)
        .with_resource_limits(limits);
    let _ = context.eval_named("fuzz-input.coffee", source);
});
