use std::cell::RefCell;
use std::path::PathBuf;

use r6502::compiler::Compiler;
use clap::Parser;
use clap::Subcommand;
use r6502::compiler::CompilerConfig;

#[derive(Subcommand, Debug)]
enum Mode {
    /// Print compiled hex values
    Hex,
    /// Print parse result of the program
    Parse
}

#[derive(Parser, Debug)]
#[command(version = "0.0.2", about = "6502 assembly compiler", long_about = None)]
struct Args {
    /// File path
    file: String,
    /// Output path
    output: Option<String>,
    /// Output mode
    #[clap(subcommand)]
    mode: Option<Mode>,
    // todo
    // add allow illegal + allow_list=hex list (should support any format)
}

fn main() -> Result<(), String> {
    let args = Args::parse();
    let input = PathBuf::from(args.file);
    let output = match args.output {
        Some(path) => PathBuf::from(path),
        None => PathBuf::from("./a.bin"),
    }; 
    
    let config = CompilerConfig {
        enable_nes: true,
        allow_illegal: false,
        allow_list: RefCell::new(vec![])
    };
    let mut compiler = Compiler::new(Some(config));
    compiler.init(input)?;

    if let Some(mode) = args.mode {
        match mode {
            Mode::Hex => {
                let hex_string = compiler.to_hex_string()?;
                print!("{}", hex_string);
            },
            Mode::Parse => print!("{}", compiler.get_parse_string()),
        }
    } else {
        compiler.run(&output)?;
        println!("Binary generated at {}", output.display());
    }

    Ok(())
}
