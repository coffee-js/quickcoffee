#![no_main]

use libfuzzer_sys::fuzz_target;
use quickcoffee::Engine;

fuzz_target!(|data: &[u8]| {
    let source = String::from_utf8_lossy(data);
    let _ = Engine::new().check_program(&source);
});
