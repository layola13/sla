//! Emit the fixture-01 structural model as SAB bytes for inspection
//! and for the coordinator's differential harness.
//!
//! Usage: `emit_fixture01 [fixture01|minimal] [output-path]`.
//! The `minimal` mode emits the golden-test module of
//! tests/golden.rs (one sig, two instructions) for byte comparison
//! against the Zig probe in `probe/`.
//! Prints the byte length, the pool contents in order, and a hex
//! dump to stdout; writes the raw bytes to the output path.

use sla_codegen_sab::encode::encode_module;
use sla_codegen_sab::fixture01::fixture01_module;
use sla_codegen_sab::ir::{FunctionSig, InstKind, Instruction, Module, Operand, PrimType};
use sla_codegen_sab::pool::build_pool;

fn minimal_module() -> Module {
    let mut func_decl = Instruction::new(InstKind::FuncDecl, 0, 0);
    func_decl.operands[0] = Operand::Symbol(0);
    func_decl.operands[1] = Operand::Func(0);
    let ret = Instruction::new(InstKind::Return, 0, 1);
    Module {
        symbols: vec!["f".to_string()],
        function_sigs: vec![FunctionSig::new(0, "f", PrimType::Void, 0)],
        const_decls: Vec::new(),
        instructions: vec![func_decl, ret],
    }
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "fixture01".to_string());
    let (module, default_path) = match mode.as_str() {
        "minimal" => (minimal_module(), "minimal.sab"),
        _ => (fixture01_module(), "fixture01.sab"),
    };
    let out_path = std::env::args()
        .nth(2)
        .unwrap_or_else(|| default_path.to_string());
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
