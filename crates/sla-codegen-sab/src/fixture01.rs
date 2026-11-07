//! Hand-lowered structural model of coordinator fixture
//! `fixtures/01_hello.sla`:
//!
//! ```sla
//! fn main() -> i32 {
//!     println("hello world");
//!     return 0;
//! }
//!
//! @test "hello world"() {
//!     if !str_eq("hello world", "hello world") { panic(101); }
//! }
//! ```
//!
//! SCOPE — read before trusting this as parity evidence. This model
//! exercises the emission trunk (`emit.rs`) and the encoder
//! (`encode.rs`) at the instruction level, using only instruction
//! forms read verbatim from sab_codegen.zig. It is NOT yet the byte
//! output of the real plugin for this fixture: the real codegen
//! expands `println` and `str_eq` through the std-surface machinery
//! (decomposition modules `format_print` / `surface_call`), which is
//! phase-2 work. The two surface calls therefore appear here in the
//! trunk's text-body call form, with the string const interned exactly
//! as the real codegen interns it. The parity claim for this slice is
//! the encoder-level one stated in `encode.rs`: same logical module
//! in, byte-identical SAB out.
//!
//! Model choices where the Zig flow was not read end-to-end:
//! - the test function's symbol is `test_hello_world` (the real
//!   `genTestDecl` naming is phase-2 reading);
//! - `main`'s recorded reg set contains only the return temp.

use crate::emit::ModuleBuilder;
use crate::ir::{FunctionKind, FunctionSig, Module, PrimType};

/// Build the fixture-01 model module.
pub fn fixture01_module() -> Module {
    let mut b = ModuleBuilder::new();

    // fn main() -> i32
    let main_sig = b.add_sig(FunctionSig::new(0, "main", PrimType::I32, 0));
    b.begin_function(main_sig);
    b.append_decl_inst(main_sig);
    // println("hello world"): the literal is interned first, exactly
    // as genPrintlnCall materializes it, then the surface call is
    // emitted in trunk text-body form.
    let label = b
        .string_literal_const_label("hello world")
        .expect("plain literal always decodes");
    b.emit_call(None, &format!("@println({label})"));
    let ret = b.new_tmp();
    b.emit_assign_int(ret, 0, false);
    b.emit_return(Some(ret));
    b.finish_function();

    // @test "hello world"
    let mut test_sig = FunctionSig::new(0, "test_hello_world", PrimType::Void, 0);
    test_sig.kind = FunctionKind::TestFunc;
    let test_sig = b.add_sig(test_sig);
    b.begin_function(test_sig);
    b.append_decl_inst(test_sig);
    let cond = b.new_tmp();
    b.emit_call(
        Some(cond),
        &format!("@str_eq({label}, {label})"),
    );
    let then_label = b.new_label("then");
    let else_label = b.new_label("else");
    b.emit_branch(cond, &then_label, &else_label);
    b.emit_label(&else_label);
    b.emit_panic_code(101);
    b.emit_label(&then_label);
    b.emit_return(None);
    b.finish_function();

    b.finish()
}
