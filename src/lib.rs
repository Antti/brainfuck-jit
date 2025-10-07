#[macro_use]
extern crate lazy_static;

#[cfg(not(target_os = "macos"))]
extern crate libc;

mod vm;
pub use crate::vm::BfJitVM;

pub fn run(code: &str) {
    let mut vm = BfJitVM::new(0x10000, 0x10000).expect("Failed to create Brainfuck JIT VM.");
    vm.compile(code);
    vm.run();
}
