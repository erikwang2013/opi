use engine_data::{load_bytes, Dictionary};
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
        Some("verify") => {
            let Some(path) = args.get(2) else {
                eprintln!("usage: opi-tools verify <file.opid>");
                std::process::exit(2);
            };
            let bytes = match std::fs::read(path) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("read {}: {e}", path);
                    std::process::exit(1);
                }
            };
            let t0 = std::time::Instant::now();
            match load_bytes(bytes) {
                Ok(d) => {
                    let elapsed = t0.elapsed();
                    println!("file: {}", path);
                    println!("checksum: ok");
                    println!("entries: {}", d.len());
                    println!("load: {:.1}ms", elapsed.as_secs_f64() * 1000.0);
                    for sample in ["hao", "wo", "n"] {
                        let top: Vec<String> =
                            d.query(sample, 3).iter().map(|e| e.word.clone()).collect();
                        println!("query \"{sample}\": {}", top.join(" "));
                    }
                }
                Err(e) => {
                    eprintln!("verify failed: {e:?}");
                    std::process::exit(1);
                }
            }
        }
        _ => {
            eprintln!("usage: opi-tools <compile|verify> ...");
            std::process::exit(2);
        }
    }
}
