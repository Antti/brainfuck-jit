mod vm;
pub use crate::vm::BfJitVM;

#[derive(thiserror::Error, Debug)]
pub enum JitVmError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Memory error: {0}")]
    MapError(std::io::Error),
    #[error("JIT VM error: {0}")]
    JitVmError(String),
    #[error("Layout error: {0}")]
    LayoutError(#[from] std::alloc::LayoutError),
    #[error("Invalid instruction: {0}")]
    InvalidInstruction(char),
}
pub type Result<T> = std::result::Result<T, JitVmError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CustomInstr {
    /// Push RSI (custom extension)
    Push,
    /// Pop RSI (custom extension)
    Pop,
    /// Move RSI to specific address (custom extension)
    MovRsi,
    /// Software breakpoint (debug)
    Debug,
    /// Return (custom extension)
    Ret,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BrainfuckInstr {
    /// Increment data pointer (>)
    IncPtr,
    /// Decrement data pointer (<)
    DecPtr,
    /// Increment byte at data pointer (+)
    IncData,
    /// Decrement byte at data pointer (-)
    DecData,
    /// Output byte at data pointer (.)
    Output,
    /// Read byte into data pointer (,)
    Input,
    /// Jump forward if zero ([)
    LoopStart,
    /// Jump backward if nonzero (])
    LoopEnd,
}

impl TryFrom<char> for BrainfuckInstr {
    type Error = JitVmError;

    fn try_from(c: char) -> Result<Self> {
        match c {
            '>' => Ok(Self::IncPtr),
            '<' => Ok(Self::DecPtr),
            '+' => Ok(Self::IncData),
            '-' => Ok(Self::DecData),
            '.' => Ok(Self::Output),
            ',' => Ok(Self::Input),
            '[' => Ok(Self::LoopStart),
            ']' => Ok(Self::LoopEnd),
            _ => Err(JitVmError::InvalidInstruction(c)),
        }
    }
}


impl BrainfuckInstr {
    pub fn from_str(src: &str) -> Result<Vec<BrainfuckInstr>> {
        src.chars().filter_map(|c| {
            match c {
                '\n' => None,
                c => Some(BrainfuckInstr::try_from(c))
            }
        }).collect()
    }
}

pub fn run(code: &[BrainfuckInstr]) -> Result<()> {
    let mut vm = BfJitVM::new(0x10000, 0x10000)?;
    vm.compile(code);
    vm.run();
    Ok(())
}
