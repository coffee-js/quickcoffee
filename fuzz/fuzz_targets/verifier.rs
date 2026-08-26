#![no_main]

use libfuzzer_sys::fuzz_target;
use quickcoffee::{Chunk, Instruction};

fuzz_target!(|data: &[u8]| {
    let code = data
        .iter()
        .map(|byte| match byte % 8 {
            0 => Instruction::Pop,
            1 => Instruction::Dup,
            2 => Instruction::Add,
            3 => Instruction::Return,
            4 => Instruction::Constant((*byte as usize) % 4),
            5 => Instruction::Jump((*byte as i8) as i32),
            6 => Instruction::MakeArray((*byte as usize) % 8),
            _ => Instruction::Call((*byte as usize) % 8),
        })
        .collect();
    let _ = Chunk {
        constants: Vec::new(),
        code,
    }
    .verify();
});
