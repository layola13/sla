//! Emit the fixture-01 structural model as SAB bytes for inspection
//! and for the coordinator's differential harness.
//!
//! Usage: `emit_fixture01 [output-path]` (default `fixture01.sab`).
//! Prints the byte length, the pool contents in order, and a hex
//! dump to stdout; writes the raw bytes to the output path.

use sla_codegen_sab::encode::encode_module;
use sla_codegen_sab::fixture01::fixture01_module;
use sla_codegen_sab::pool::build_pool;

fn main() {
    let out_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "fixture01.sab".to_string());
    let module = fixture01_module();
    let pool = build_pool(&module);
    let bytes = encode_module(&module).expect("encode fixture01 model");
    std::fs::write(&out_path, &bytes).expect("write output file");
    println!("wrote {} ({} bytes)", out_path, bytes.len());
    println!("symbols: {:?}", module.symbols);
    println!("pool ({} entries):", pool.len());
    for (i, entry) in pool.entries().iter().enumerate() {
        println!("  [{i}] {entry:?}");
    }
    println!("instructions: {}", module.instructions.len());
    println!("hex:");
    for chunk in bytes.chunks(16) {
        let hex: Vec<String> = chunk.iter().map(|b| format!("{b:02x}")).collect();
        println!("  {}", hex.join(" "));
    }
}
