use opi_tools::compiler::{compile_file, parse_dict};
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("compile") => {
            let (Some(input), Some(output)) = (args.get(2), args.get(3)) else {
                eprintln!("usage: opi-tools compile <input.tsv|dict.yaml> <output.opid>");
                std::process::exit(2);
            };
            let text = match std::fs::read_to_string(input) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("read {}: {e}", input);
                    std::process::exit(1);
                }
            };
            let entries = parse_dict(&text);
            println!("input lines: {}", text.lines().count());
            println!("kept entries: {}", entries.len());
            if let Err(e) = compile_file(Path::new(input), Path::new(output)) {
                eprintln!("{e}");
                std::process::exit(1);
            }
            let size = std::fs::metadata(output).map(|m| m.len()).unwrap_or(0);
            println!("wrote {} ({} bytes)", output, size);
        }
        _ => {
            eprintln!("usage: opi-tools <compile|verify> ...");
            std::process::exit(2);
        }
    }
}
