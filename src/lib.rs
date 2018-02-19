#![feature(allocator_api)]
#![feature(pointer_methods)]
#[macro_use]
extern crate lazy_static;

#[cfg(not(target_os = "macosx"))]
extern crate libc;

mod vm;
pub use vm::BfJitVM;

pub fn run(code: &str) {
    let mut vm = BfJitVM::new(0x10000, 0x10000).expect("Failed to create Brainfuck JIT VM.");
    vm.compile(code);
    vm.run();
}
