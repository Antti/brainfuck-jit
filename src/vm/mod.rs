unsafe extern "C" {
    fn getchar() -> i32;
    fn putchar_unlocked(ch: i32) -> i32;
}

mod jitmem;

use crate::{BrainfuckInstr, CustomInstr, Result};
use jitmem::JitMemory;

const STACK_ALIGNMENT_BLOCKS: usize = 16; // 128 bytes red zone

// Possible optimizations:
// * multiple + and - as add/sub byte [rsi], X
// * we can save getchar and putchar in some registers in the prolog
//   and then we can call them directly (or move to rax if necessary)
#[cfg(windows)]
const PUTCHAR_CODE: &[u8] = &[
    0x0f, 0xb6, 0x0e, // movzx ecx, byte [rsi]
    0x48, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, // mov rax, putchar_unlocked
    0xff, 0xd0, // call rax
];

#[cfg(windows)]
const GETCHAR_CODE: &[u8] = &[
    0x48, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, // mov rax, getchar
    0xff, 0xd0, // call rax
    0x88, 0x06, // mov byte [rsi], al
];

#[cfg(unix)]
const PUTCHAR_CODE: &[u8] = &[
    0x48, 0x0f, 0xb6, 0x3e, // movzx rdi, byte [rsi]
    0x48, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, // mov rax, putchar_unlocked
    0x56, // push rsi
    0xff, 0xd0, // call rax
    0x5e, // pop rsi
];

#[cfg(unix)]
const GETCHAR_CODE: &[u8] = &[
    0x48, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, // mov rax, getchar
    0x56, // push rsi
    0xff, 0xd0, // call rax
    0x5e, // pop rsi
    0x88, 0x06, // mov byte [rsi], al
];

pub fn custom_instr_to_bin_code(ch: CustomInstr) -> &'static [u8] {
    match ch {
        CustomInstr::Debug => &[0xCC], // int3 breakpoint
        CustomInstr::Push => &[0x56], // push rsi
        CustomInstr::MovRsi => &[0x48, 0xbe, 0, 0, 0, 0, 0, 0, 0, 0], // mov rsi, addr
        CustomInstr::Pop => &[0x5e], // pop rsi
        CustomInstr::Ret => &[0xc3], // ret
    }
}

pub fn instr_to_bin_code(ch: BrainfuckInstr) -> &'static [u8] {
    match ch {
        BrainfuckInstr::IncPtr => &[0x48, 0xff, 0xc6], // inc rsi
        BrainfuckInstr::DecPtr => &[0x48, 0xff, 0xce], // dec rsi
        BrainfuckInstr::IncData => &[0xfe, 0x06], // inc byte [rsi]
        BrainfuckInstr::DecData => &[0xfe, 0x0e], // dec byte [rsi]
        BrainfuckInstr::LoopStart => &[
            0x80, 0x3e, 0x00,       // cmp byte [rsi], al
            0x0f, 0x84, 0, 0, 0, 0, // je addr
        ],
        BrainfuckInstr::LoopEnd => &[
            0x80, 0x3e, 0x00,       // cmp byte [rsi], al
            0x0f, 0x85, 0, 0, 0, 0, // jne addr
        ],
        BrainfuckInstr::Output => PUTCHAR_CODE,
        BrainfuckInstr::Input => GETCHAR_CODE
    }
}

pub struct BfJitVM {
    code_mem: JitMemory,
    data_mem: Vec<u8>,
}

impl BfJitVM {
    pub fn new(code_size: usize, data_size: usize) -> Result<BfJitVM> {
        let mut jit_mem = JitMemory::alloc(code_size)?;
        jit_mem.write_at(&mut 0, custom_instr_to_bin_code(CustomInstr::Ret));
        let data_mem = vec![0; data_size];

        Ok(BfJitVM {
            code_mem: jit_mem,
            data_mem,
        })
    }

    pub fn compile(&mut self, code: &[BrainfuckInstr]) -> bool {
        if !self.check_before_compilation(code) {
            return false;
        }
        self.compile_helper(code);
        true
    }

    pub fn run(&mut self) {
        // zero out the data memory
        for it in self.data_mem.iter_mut() {
            *it = 0;
        }

        let jit_function = self.code_mem.as_function();
        eprintln!("JitVM: Running code from addr 0x{:x}", jit_function as u64);
        eprintln!("JitVM: VM memory at 0x{:x}", self.data_mem.as_ptr() as u64);
        eprintln!("-------------------");
        jit_function();
    }

    // --------------- compiler -------------------
    fn check_before_compilation(&self, code: &[BrainfuckInstr]) -> bool {
        let mut required_code_mem = 0;
        let mut opened_loops = 0;

        for &instr in code {
            match instr {
                BrainfuckInstr::IncPtr | BrainfuckInstr::DecPtr | BrainfuckInstr::IncData | BrainfuckInstr::DecData | BrainfuckInstr::Output | BrainfuckInstr::Input => {
                    required_code_mem += instr_to_bin_code(instr).len();
                }
                BrainfuckInstr::LoopStart => {
                    opened_loops += 1;
                    required_code_mem += instr_to_bin_code(instr).len();
                }
                BrainfuckInstr::LoopEnd => {
                    if opened_loops == 0 {
                        eprintln!("JitCompiler: Error: found ']' without corresponding '['.");
                        return false;
                    }
                    opened_loops -= 1;
                    required_code_mem += instr_to_bin_code(instr).len();
                }
            }
        }

        if opened_loops > 0 {
            eprintln!("JitCompiler: Error: too many ']'.");
            return false;
        }

        // note: this var occurs also in compile_helper!
        // prolog
        required_code_mem += custom_instr_to_bin_code(CustomInstr::Push).len() * STACK_ALIGNMENT_BLOCKS;
        required_code_mem += custom_instr_to_bin_code(CustomInstr::MovRsi).len();
        // epilog
        required_code_mem += custom_instr_to_bin_code(CustomInstr::Pop).len() * STACK_ALIGNMENT_BLOCKS;
        required_code_mem += custom_instr_to_bin_code(CustomInstr::Ret).len();
        let vm_code_buffer_size = self.code_mem.len();
        if required_code_mem > vm_code_buffer_size {
            eprintln!(
                "JitCompiler: Error: code requires {} bytes, but VM has a buffer of {} \
                      bytes.",
                required_code_mem, vm_code_buffer_size
            );
            return false;
        }

        eprintln!(
            "JitCompiler: Warning: did not validate if VM memory buffer is big enough and \
                  if program accesses memory beyond its boundaries."
        );
        eprintln!("JitCompiler: Warning: assuming that getchar and putchar always succeeds.");
        true
    }

    fn compile_helper(&mut self, code: &[BrainfuckInstr]) {
        let mut ip = 0;
        // for debugging:
        // self.code_mem.write_at(&mut ip, INSTR_TO_BIN_CODE[&'d']);
        for _ in 0..STACK_ALIGNMENT_BLOCKS {
            // space on the stack for calling putchar/getchar + preserving rsi
            self.code_mem.write_at(&mut ip, custom_instr_to_bin_code(CustomInstr::Push));
        }
        self.code_mem.write_at(&mut ip, custom_instr_to_bin_code(CustomInstr::MovRsi));
        let addr_size = 8;
        self.code_mem
            .patch_addr_u64(ip - addr_size, self.data_mem.as_ptr() as u64);
        let (mut ip, chars_processed) = self.compile_loop_body(code, ip);
        assert_eq!(chars_processed, code.len());
        for _ in 0..STACK_ALIGNMENT_BLOCKS {
            self.code_mem.write_at(&mut ip, custom_instr_to_bin_code(CustomInstr::Pop));
        }
        self.code_mem.write_at(&mut ip, custom_instr_to_bin_code(CustomInstr::Ret));
        eprintln!("JitCompiler: compilation resulted in {} bytes.", ip);
        eprintln!("-------------------");
    }

    // returns (new ip, code chars processed)
    fn compile_loop_body(&mut self, code: &[BrainfuckInstr], begin_ip: usize) -> (usize, usize) {
        let mut ip = begin_ip;
        let mut chars_processed = 0;
        while chars_processed < code.len() {
            let instr = code[chars_processed];
            match instr {
                BrainfuckInstr::IncPtr | BrainfuckInstr::DecPtr | BrainfuckInstr::IncData | BrainfuckInstr::DecData => {
                    self.code_mem.write_at(&mut ip, instr_to_bin_code(instr));
                }
                BrainfuckInstr::Output => {
                    self.code_mem.write_at(&mut ip, instr_to_bin_code(instr));
                    let putchar_addr_offset = if cfg!(unix) {
                        12
                    } else if cfg!(windows) {
                        10
                    } else {
                        panic!("Unsupported platform");
                    };
                    self.code_mem
                        .patch_addr_u64(ip - putchar_addr_offset, putchar_unlocked as u64);
                }
                BrainfuckInstr::Input => {
                    self.code_mem.write_at(&mut ip, instr_to_bin_code(instr));
                    let getchar_addr_offset = if cfg!(unix) {
                        14
                    } else if cfg!(windows) {
                        12
                    } else {
                        panic!("Unsupported platform");
                    };
                    self.code_mem
                        .patch_addr_u64(ip - getchar_addr_offset, getchar as u64);
                }
                BrainfuckInstr::LoopStart => {
                    self.code_mem.write_at(&mut ip, instr_to_bin_code(instr));
                    let addr_to_patch = ip - 4;
                    let (new_ip, new_cp) = self.compile_loop_body(&code[chars_processed + 1..], ip);
                    let offset = new_ip - ip;
                    assert!(offset > 0 && offset < (1 << 31));
                    self.code_mem.patch_addr_i32(addr_to_patch, offset as i32);
                    ip = new_ip;
                    chars_processed += new_cp;
                }
                BrainfuckInstr::LoopEnd => {
                    self.code_mem.write_at(&mut ip, instr_to_bin_code(instr));
                    let addr_to_patch = ip - 4;
                    let offset = ip - begin_ip;
                    assert!(offset > 0 && offset < (1 << 31));
                    self.code_mem
                        .patch_addr_i32(addr_to_patch, -(offset as i32));
                }
            }

            chars_processed += 1;
            if instr == BrainfuckInstr::LoopEnd {
                break;
            }
        }

        (ip, chars_processed)
    }
}
