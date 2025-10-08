extern crate bf_jit;

use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::process::exit;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} file.bf", args[0]);
        exit(1);
    }

    let mut f = File::open(&args[1])?;
    let mut contents = String::new();
    f.read_to_string(&mut contents)?;

    let code = bf_jit::BrainfuckInstr::from_str(&contents)?;
    bf_jit::run(&code)?;
    Ok(())
}
