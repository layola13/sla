//! R-3 arena-slice regression tests: the two defects this slice
//! closes, each with its corpus twin named in the test.
//!
//! 1. Borrow-typed parameters (`x: &T`) on PLAIN calls: the
//!    simplified signature checker stripped the argument's borrow
//!    and compared against the un-stripped parameter type, so every
//!    `count(&v)` against `fn count(x: &Vec<String>)` was rejected
//!    with "expects borrow, actual borrow". Corpus twins:
//!    test_arena_ok_borrow_pass / test_arena_ok_clone_out (the
//!    defect is arena-independent — the bare out-of-arena form is
//!    the red-before control).
//! 2. Tainted-closure invocation: a closure that captured an arena
//!    root was registered as tainted, but the reachability walk
//!    only examined `call.callee` (immediately-invoked form), never
//!    the bare-name `call.func_name` shape, so `return f(1)` on a
//!    capturing closure passed silently. Corpus twin:
//!    test_error_arena_closure_capture (expect ArenaEscape).

use crate::checker::Checker;
use crate::diag::ErrorKind;

fn check_src(src: &str) -> Result<(), (ErrorKind, String)> {
    let mut parser =
        sla_parser::Parser::with_options(src, sla_parser::ParserOptions::default());
    let root = parser.parse_program().map_err(|e| {
        (
            ErrorKind::CompileError,
            format!("parse: {:?}", e.kind),
        )
    })?;
    let sla_ast::node::Node::Program(prog) = root else {
        panic!("root not program");
    };
    let mut checker = Checker::new();
    checker
        .check_program(&prog)
        .map_err(|d| (d.kind, d.message))
}

const BORROW_DECL: &str =
    "fn count(x: &Vec<String>) -> i32 { return 7; }\n";

#[test]
fn r3_borrow_typed_param_plain_call_outside_arena() {
    // Bare form of the defect (no arena anywhere).
    let src = format!(
        "{BORROW_DECL}fn g() -> i32 {{\n    let v: Vec<String> = Vec::new();\n    let n = count(&v);\n    return n;\n}}\n"
    );
    assert_eq!(check_src(&src), Ok(()));
}

#[test]
fn r3_borrow_typed_param_inside_arena() {
    // Corpus twin: test_arena_ok_borrow_pass.
    let src = format!(
        "{BORROW_DECL}fn ok_borrow_pass() -> i32 {{\n    arena {{\n        let v: Vec<String> = Vec::new();\n        let n = count(&v);\n        if n != 7 {{ return 1; }};\n    }}\n    return 0;\n}}\n"
    );
    assert_eq!(check_src(&src), Ok(()));
}

#[test]
fn r3_borrow_typed_param_clone_out() {
    // Corpus twin: test_arena_ok_clone_out (sanctioned export:
    // borrow in, fresh value out).
    let src = "fn clone_vec(x: &Vec<String>) -> Vec<String> {\n    let out: Vec<String> = Vec::new();\n    return out;\n}\nfn ok_clone_out() -> Vec<String> {\n    arena {\n        let v: Vec<String> = Vec::new();\n        return clone_vec(&v);\n    }\n}\n";
    assert_eq!(check_src(src), Ok(()));
}

#[test]
fn r3_borrow_typed_param_mismatch_still_rejected() {
    // Control: the new arm must not become a blanket pass — a
    // borrow of the WRONG inner type still mismatches.
    let src = format!(
        "{BORROW_DECL}fn g() -> i32 {{\n    let v: Vec<i32> = Vec::new();\n    let n = count(&v);\n    return n;\n}}\n"
    );
    let (kind, _) = check_src(&src).expect_err("must reject");
    assert_eq!(kind, ErrorKind::TypeMismatch);
}

#[test]
fn r3_borrow_typed_param_bare_value_still_rejected() {
    // Control: a by-value argument to a borrow-typed parameter is
    // not an implicit borrow on the plain path (Zig's arm requires
    // the argument type itself to be a borrow).
    let src = format!(
        "{BORROW_DECL}fn g() -> i32 {{\n    let v: Vec<String> = Vec::new();\n    let n = count(v);\n    return n;\n}}\n"
    );
    let (kind, _) = check_src(&src).expect_err("must reject");
    assert_eq!(kind, ErrorKind::TypeMismatch);
}

#[test]
fn r3_closure_capture_invocation_escapes() {
    // Corpus twin: test_error_arena_closure_capture.
    let src = "fn neg_closure_capture() -> i32 {\n    arena {\n        let v: Vec<String> = Vec::new();\n        let f = |x: i32| x + (len(v) as i32);\n        return f(1);\n    }\n}\n";
    let (kind, _) = check_src(src).expect_err("must reject");
    assert_eq!(kind, ErrorKind::ArenaEscape);
}

#[test]
fn r3_closure_capture_return_escapes() {
    // Same taint, returned uninvoked: the closure VALUE itself
    // carries the capture out (Identifier reach arm).
    let src = "fn neg_closure_return() -> i32 {\n    arena {\n        let v: Vec<String> = Vec::new();\n        let f = |x: i32| x + (len(v) as i32);\n        return f;\n    }\n}\n";
    let (kind, _) = check_src(src).expect_err("must reject");
    assert_eq!(kind, ErrorKind::ArenaEscape);
}

#[test]
fn r3_impl_self_literal_and_return() {
    // Corpus twin: test_unit_impl_static_methods — `Self { ... }`
    // constructs the impl target and the `-> Self` return accepts
    // it (checker-side Self resolution under check_impl).
    let src = "struct S { a: i32 }\nimpl S {\n    fn make(a: i32) -> Self { return Self { a: a }; }\n    fn get(self) -> i32 { return self.a; }\n}\nfn g() -> i32 {\n    let s = S::make(3);\n    return s.get();\n}\n";
    assert_eq!(check_src(src), Ok(()));
}

#[test]
fn r3_plain_closure_invocation_stays_legal() {
    // Control: a non-capturing arena-local closure invoked in the
    // domain is ordinary traffic, not an escape. (The invocation
    // result stays inside the domain; returning it directly is the
    // tainted shape's twin and is judged by the return gate only
    // when the closure captures.)
    let src = "fn ok_closure() -> i32 {\n    arena {\n        let v: Vec<String> = Vec::new();\n        let f = |x: i32| x + 1;\n        let n = f(1);\n        if n != 2 { return 1; };\n    }\n    return 0;\n}\n";
    assert_eq!(check_src(src), Ok(()));
}

#[test]
fn r3_typed_macro_params_positive() {
    // Corpus twin: test_unit_typed_macro_params (statement form,
    // mixed typed/untyped params, body sees declared types).
    // Call sites judged through `sla check` on the corpus file
    // (r3work ledger): the in-process harness's macro-argument
    // consumption diverges from the standalone binary for the
    // leading-output-as-place form — registered in RESULT as a
    // harness/binary divergence, not patched over here.
    let src = "macro add_ctx_into(out: i64, x: i64, scale) { out = (x + scale) * 2; }\nfn helper_i64(v: i64) -> i64 { return v + 1; }\nmacro bump_into(out: i64, x: i64) { out = helper_i64(x); }\nfn f() -> i64 {\n    let r1: i64 = 0;\n    add_ctx_into(r1, 10, 5);\n    let r2: i64 = 0;\n    bump_into(r2, 41);\n    return r1 + r2;\n}\n";
    assert_eq!(check_src(src), Ok(()));
}

#[test]
fn r3_typed_macro_arg_mismatch() {
    // Corpus twin: test_error_typed_macro_arg_mismatch.
    let src = "macro double_into(out: i64, x: i64) { out = x + x; }\nfn f() -> i64 {\n    let r: i64 = 0;\n    let bad: f64 = 1.5;\n    double_into(r, bad);\n    return r;\n}\n";
    let (kind, _) = check_src(src).expect_err("must reject");
    assert_eq!(kind, ErrorKind::TypeMismatch);
}

#[test]
fn r3_typed_macro_body_mismatch() {
    // Corpus twin: test_error_typed_macro_body_mismatch — the
    // declared type is visible inside the body even before any
    // call-site argument check could fire.
    let src = "macro bad_body(out: i64, x: i64) { let s: f64 = 1.5; out = s; }\nfn f() -> i64 {\n    let r: i64 = 0;\n    bad_body(r, 2);\n    return r;\n}\n";
    let (kind, _) = check_src(src).expect_err("must reject");
    assert_eq!(kind, ErrorKind::TypeMismatch);
}

#[test]
fn r3_macro_expr_output_positive() {
    // Corpus twin: test_unit_macro_expr_output (J-4).
    let src = "macro doubled(out_result: i64, x: i64) { out_result = x + x; }\nfn compute(v: i64) -> i64 {\n    let a = doubled(v);\n    let b = doubled(a) + 1;\n    return b;\n}\n";
    assert_eq!(check_src(src), Ok(()));
}

#[test]
fn r3_macro_expr_output_untyped() {
    // Corpus twin: test_error_macro_expr_output_untyped.
    let src = "macro doubled(out_result, x: i64) { out_result = x + x; }\nfn f() -> i64 {\n    let y = doubled(3);\n    return y;\n}\n";
    let (kind, _) = check_src(src).expect_err("must reject");
    assert_eq!(kind, ErrorKind::MacroExpressionOutputUntyped);
}

#[test]
fn r3_macro_expr_output_name_not_output() {
    // Corpus twin: test_error_macro_expr_output_name — `result`
    // is not a leading-output name, so omission is an arity error.
    let src = "macro doubled(result: i64, x: i64) { result = x + x; }\nfn f() -> i64 {\n    let y = doubled(3);\n    return y;\n}\n";
    let (kind, _) = check_src(src).expect_err("must reject");
    assert_eq!(kind, ErrorKind::InvalidArgsCount);
}

#[test]
fn r3_fallible_sugar_return_desugars() {
    // A_min producer: `-> FsReadBuffer!`-shaped returns on plain
    // functions parse and check (desugar to Result<T, i32>).
    let src = "struct B { raw: u64 }\nfn make() -> B! { return Ok(B { raw: 1 }); }\nfn f() -> u64 {\n    match make() {\n        Ok(b) => { return b.raw; },\n        Err(e) => { return 0; },\n    };\n}\n";
    assert_eq!(check_src(src), Ok(()));
}

#[test]
fn r3_fallible_sugar_param_rejected() {
    // Corpus twin: neg_fallible_sugar_param.
    let src = "fn sugared_param(x: u64!) -> u64 { return 0; }\n";
    let (kind, _) = check_src(src).expect_err("must reject");
    assert_eq!(kind, ErrorKind::FallibleSugarParam);
}

#[test]
fn r3_borrow_origin_positive_direct_return() {
    // The annotated producer returns its own view directly.
    let src = "struct B { raw: u64 }\nfn data(buffer: &B) -> &[u8][buffer] { return __slice_view(0, 0); }\nfn f() -> i32 { return 0; }\n";
    assert_eq!(check_src(src), Ok(()));
}

#[test]
fn r3_borrow_annotation_invalid() {
    // Corpus twin: neg_borrow_annotation_bad.
    let src = "struct B { raw: u64 }\nfn bad(owner: &B) -> &[u8][stranger] { return __slice_view(0, 0); }\n";
    let (kind, _) = check_src(src).expect_err("must reject");
    assert_eq!(kind, ErrorKind::BorrowAnnotationInvalid);
}

#[test]
fn r3_borrow_escape_return() {
    // Corpus twin: neg_borrow_escape_return.
    let src = "struct B { raw: u64 }\nfn data(buffer: &B) -> &[u8][buffer] { return __slice_view(0, 0); }\nfn bad(owner: &B) -> &[u8] {\n    let view = data(owner);\n    return view;\n}\n";
    let (kind, _) = check_src(src).expect_err("must reject");
    assert_eq!(kind, ErrorKind::BorrowEscape);
}

#[test]
fn r3_borrow_escape_store() {
    // Corpus twin: neg_borrow_escape_store.
    let src = "struct B { raw: u64 }\nstruct Holder { data: &[u8] }\nfn data(buffer: &B) -> &[u8][buffer] { return __slice_view(0, 0); }\nfn bad() -> u64 {\n    let owner = B { raw: 7 };\n    let view = data(&owner);\n    let h = Holder { data: view };\n    return h.data.len();\n}\n";
    let (kind, _) = check_src(src).expect_err("must reject");
    assert_eq!(kind, ErrorKind::BorrowEscape);
}

#[test]
fn r3_borrow_origin_not_binding() {
    // Corpus twin: neg_borrow_origin_not_binding.
    let src = "struct B { raw: u64 }\nfn data(buffer: &B) -> &[u8][buffer] { return __slice_view(0, 0); }\nfn bad() -> u64 {\n    let view = data(B { raw: 7 });\n    return view.len();\n}\n";
    let (kind, _) = check_src(src).expect_err("must reject");
    assert_eq!(kind, ErrorKind::BorrowOriginNotABinding);
}

#[test]
fn r3_borrow_use_after_move_owner() {
    // Corpus twin: neg_borrow_move_owner.
    let src = "struct B { raw: u64 }\nfn data(buffer: &B) -> &[u8][buffer] { return __slice_view(0, 0); }\nfn bad() -> u64 {\n    let owner = B { raw: 7 };\n    let view = data(&owner);\n    let moved = owner;\n    return view.len();\n}\n";
    let (kind, _) = check_src(src).expect_err("must reject");
    assert_eq!(kind, ErrorKind::BorrowUseAfterInvalidation);
}

#[test]
fn r3_borrow_use_after_unknown_call() {
    // Corpus twin: neg_borrow_unknown_call (conservative default).
    let src = "struct B { raw: u64 }\nfn data(buffer: &B) -> &[u8][buffer] { return __slice_view(0, 0); }\nfn touch(b: &B) -> u64 { return b.raw; }\nfn bad() -> u64 {\n    let owner = B { raw: 7 };\n    let view = data(&owner);\n    let t = touch(&owner);\n    return view.len();\n}\n";
    let (kind, _) = check_src(src).expect_err("must reject");
    assert_eq!(kind, ErrorKind::BorrowUseAfterInvalidation);
}

#[test]
fn r3_borrow_use_after_free() {
    // Corpus twin: neg_borrow_use_after_free.
    let src = "struct B { raw: u64 }\nfn data(buffer: &B) -> &[u8][buffer] { return __slice_view(0, 0); }\nfn free_fn(^buffer: B) -> i32 { return 0; }\nfn bad() -> u64 {\n    let owner = B { raw: 7 };\n    let view = data(&owner);\n    let rc = free_fn(^owner);\n    return view.len();\n}\n";
    let (kind, _) = check_src(src).expect_err("must reject");
    assert_eq!(kind, ErrorKind::BorrowUseAfterInvalidation);
}

#[test]
fn r3_borrow_merge_origins() {
    // Corpus twin: neg_borrow_merge_origins.
    let src = "struct B { raw: u64 }\nfn data(buffer: &B) -> &[u8][buffer] { return __slice_view(0, 0); }\nfn bad(flag: bool) -> u64 {\n    let a = B { raw: 7 };\n    let b = B { raw: 9 };\n    let va = data(&a);\n    let vb = data(&b);\n    let v = va;\n    if flag {\n        v = vb;\n    };\n    return v.len();\n}\n";
    let (kind, _) = check_src(src).expect_err("must reject");
    assert_eq!(kind, ErrorKind::BorrowOriginMerge);
}

#[test]
fn r3_go_positive_forms() {
    let src = "fn worker(x: u64) {}\nfn defer_target() {}\nfn main2() -> i32 { let h = go worker(1); defer defer_target(); return 0; }";
    assert_eq!(check_src(src), Ok(()));
}

#[test]
fn r3_go_async_callee() {
    let src = "async fn w() {}\nfn m() -> i32 { let h = go w(); return 0; }";
    let (kind, _) = check_src(src).expect_err("must reject");
    assert_eq!(kind, ErrorKind::GoAsyncCallee);
}

#[test]
fn r3_go_nonvoid_callee() {
    let src = "fn w() -> i32 { return 1; }\nfn m() -> i32 { let h = go w(); return 0; }";
    let (kind, _) = check_src(src).expect_err("must reject");
    assert_eq!(kind, ErrorKind::GoNonVoidCallee);
}

#[test]
fn r3_go_arity() {
    let src = "fn w(a: u64, b: u64) {}\nfn m() -> i32 { let h = go w(1, 2); return 0; }";
    let (kind, _) = check_src(src).expect_err("must reject");
    assert_eq!(kind, ErrorKind::GoArity);
}

#[test]
fn r3_go_closure_callee() {
    let src = "fn m() -> i32 { let f = |x: u64| x + 1; let h = go f(2); return 0; }";
    let (kind, _) = check_src(src).expect_err("must reject");
    assert_eq!(kind, ErrorKind::GoClosureCallee);
}

#[test]
fn r3_go_method_callee() {
    let src = "struct C { n: u64 }\nimpl C { fn bump(self) {} }\nfn m() -> i32 { let c = C { n: 0 }; let h = go c.bump(); return 0; }";
    let (kind, _) = check_src(src).expect_err("must reject");
    assert_eq!(kind, ErrorKind::GoMethodCallee);
}

#[test]
fn r3_go_borrow_payload() {
    let src = "struct B { r: &u64 }\nfn w(b: *B) {}\nfn m() -> i32 { let cell: u64 = 0; let bb = B { r: &cell }; let h = go w(&bb); return 0; }";
    let (kind, _) = check_src(src).expect_err("must reject");
    assert_eq!(kind, ErrorKind::GoBorrowPayload);
}

#[test]
fn r3_defer_noncall() {
    let src = "fn m() -> i32 { let x = 1; defer x; return 0; }";
    let (kind, _) = check_src(src).expect_err("must reject");
    assert_eq!(kind, ErrorKind::DeferNonCall);
}

#[test]
fn r3_go_async_select() {
    let src = "async fn w(ch: Chan<u64>) -> i32 { select { case let (v, closed) = ch.recv() { return 1; } } return 0; }";
    let (kind, _) = check_src(src).expect_err("must reject");
    assert_eq!(kind, ErrorKind::GoAsyncSelect);
}

#[test]
fn r3_select_positive() {
    let src = "fn poll(ch: Chan<u64>) -> bool { select { case let (v, closed) = ch.recv() { return true; } default { return false; } } }";
    assert_eq!(check_src(src), Ok(()));
}

#[test]
fn r3_go_arena_spawn() {
    // Corpus twin: test_error_arena_go_spawn.
    let src = "fn worker(x: Vec<String>) -> i32 { return 0; }\nfn neg() -> i32 { arena { let v: Vec<String> = Vec::new(); let h = go worker(v); } return 0; }";
    let (kind, _) = check_src(src).expect_err("must reject");
    assert_eq!(kind, ErrorKind::ArenaGoroutine);
}

#[test]
fn r3_hook_unknown() {
    let src = "@check_hook no_such_hook;\nfn f() -> i64 { return 0; }";
    let (kind, _) = check_src(src).expect_err("must reject");
    assert_eq!(kind, ErrorKind::UnknownCheckHook);
}

#[test]
fn r3_hook_audit_positive() {
    let src = "@check_hook macro_param_audit;\nmacro add_into(out: i64, x: i64, y: i64) { out = x + y; }";
    assert_eq!(check_src(src), Ok(()));
}

#[test]
fn r3_hook_audit_unused() {
    let src = "@check_hook macro_param_audit;\nmacro add_into(out: i64, x: i64, y: i64) { out = x + x; }";
    let (kind, _) = check_src(src).expect_err("must reject");
    assert_eq!(kind, ErrorKind::MacroParamUnused);
}

#[test]
fn r3_hook_audit_shadowed() {
    // The body's `let x` shadows the parameter; the audit must
    // still fire for parameter x.
    let src = "@check_hook macro_param_audit;\nmacro add_into(out: i64, x: i64) { let x: i64 = 5; out = x + x; }";
    let (kind, _) = check_src(src).expect_err("must reject");
    assert_eq!(kind, ErrorKind::MacroParamUnused);
}

#[test]
fn r3_hook_absent_no_audit() {
    // Without the hook declaration the same macro checks clean.
    let src = "macro add_into(out: i64, x: i64, y: i64) { out = x + x; }";
    assert_eq!(check_src(src), Ok(()));
}

#[test]
fn r3_sublanguage_unbound() {
    let src = "@sublanguage capsule;\nfn f() -> i64 { return 0; }";
    let (kind, _) = check_src(src).expect_err("must reject");
    assert_eq!(kind, ErrorKind::SublanguageNotBound);
}

#[test]
fn r3_shader_positive() {
    let src = "@sublanguage shader;\nshader Base {\n    priority 0;\n    @global struct Camera { view_proj: mat4; }\n    @input position: vec3;\n    @input uv: vec2;\n    @param base_color: vec4;\n    @param tex: sampler2D;\n    var v_uv: vec2;\n    vertex {\n        v_uv = uv;\n        clip_position = Camera.view_proj * vec4(position, 1.0);\n    }\n    fragment {\n        frag_color = tex.get(v_uv) * base_color;\n    }\n}";
    assert_eq!(check_src(src), Ok(()));
}

#[test]
fn r3_shader_dimension() {
    let src = "@sublanguage shader;\nshader Bad {\n    @param a: vec2;\n    @param b: vec3;\n    var v: vec3;\n    init {\n        v = a + b;\n    }\n}";
    let (kind, _) = check_src(src).expect_err("must reject");
    assert_eq!(kind, ErrorKind::ShaderDimensionMismatch);
}

#[test]
fn r3_shader_duplicate() {
    let src = "@sublanguage shader;\nshader Bad {\n    @param tint: vec4;\n    @param tint: vec4;\n}";
    let (kind, _) = check_src(src).expect_err("must reject");
    assert_eq!(kind, ErrorKind::ShaderDuplicateName);
}

#[test]
fn r3_shader_stage_violation() {
    let src = "@sublanguage shader;\nshader Bad {\n    @input position: vec3;\n    fragment {\n        frag_color = vec4(position, 1.0);\n    }\n}";
    let (kind, _) = check_src(src).expect_err("must reject");
    assert_eq!(kind, ErrorKind::ShaderStageViolation);
}

#[test]
fn r3_shader_overflow() {
    let src = "@sublanguage shader;\nshader Bad {\n    var n: i32;\n    init {\n        n = 99999999999999999999999999;\n    }\n}";
    let (kind, _) = check_src(src).expect_err("must reject");
    assert_eq!(kind, ErrorKind::Overflow);
}

#[test]
fn r3_shader_unregistered_is_not_a_decl() {
    // Without registration the file must fail (any parse-level
    // rejection; the Zig caliber names ExpectedDeclaration).
    let src = "shader Base {\n    priority 0;\n}\nfn f() -> i64 { return 0; }";
    assert!(check_src(src).is_err());
}

#[test]
fn r3_generic_field_access_substitutes() {
    // Field access on a generic instantiation substitutes the
    // struct's parameters by the receiver's arguments.
    let src = "struct Query<T> { value: T, }\nstruct Pos { x: i64, }\nfn get(q: Query<Pos>) -> i64 { return q.value.x; }";
    check_src(src).expect("generic field access must check");
}

#[test]
fn r3_generic_call_substitutes_params_and_return() {
    // Explicitly generic call: parameter AND return types are
    // substituted by the call's generic arguments.
    let src = "struct Pair2<A, B> { p0: A, p1: B, }\nstruct Pos { x: i64, }\nstruct Vel { x: i64, }\nfn make2<A, B>(p0: A, p1: B) -> Pair2<A, B> { return Pair2<A, B> { p0: p0, p1: p1 }; }\nfn main() -> i32 { let r = make2<Pos, Vel>(Pos { x: 1 }, Vel { x: 2 }); if r.p0.x != 1 { return 1; }; return 0; }";
    check_src(src).expect("generic call substitution must check");
}

#[test]
fn r3_var_param_out_shape_checks() {
    // Corpus test_error_var_param_phase2: a `var` parameter is an
    // out-parameter; `fill(var x)` initializes the caller's `var`
    // binding, so reading it afterwards is not UseBeforeInit. (The
    // file is named test_error but carries no expect line; the
    // plain-parameter control below keeps the UseBeforeInit gate.)
    let src = "fn fill(var x: i32) -> void { x = 1; }\nfn main() -> i32 { var x: i32; fill(var x); return x; }";
    check_src(src).expect("var out-parameter shape must check");
}

#[test]
fn r3_plain_param_does_not_initialize_caller_var() {
    // Control: with a PLAIN parameter the same program must still
    // fail — the call does not write back.
    let src = "fn fill(x: i32) -> void { x = 1; }\nfn main() -> i32 { var x: i32; fill(x); return x; }";
    let err = check_src(src).expect_err("plain param must not initialize");
    assert_eq!(err.0, ErrorKind::UseBeforeInit);
}

#[test]
fn r3_extern_variadic_decl_and_call_check() {
    // Corpus test_unit_variadic_extern_call (declaration face):
    // an extern fn may end with a `...` marker and be called with
    // more arguments than fixed parameters.
    let src = "extern \"C\" { fn sprintf(buf: i64, fmt: i64, ...) -> i32; }\nfn main() -> i32 { let r = sprintf(0, 0, 1, 2); return r; }";
    check_src(src).expect("variadic extern call must check");
}

#[test]
fn r3_overload_block_operator_dispatch_checks() {
    // Corpus test_unit_overload_add / demos 312: an
    // `@overload Vec2 { fn +(self: Vec2, ...) }` block registers
    // the operator; `a + b` checks against it.
    let src = "struct Vec2 { x: i32, y: i32, }\n@overload Vec2 { fn +(self: Vec2, other: Vec2) -> Vec2 { return Vec2 { x: self.x + other.x, y: self.y + other.y }; } }\nfn main() -> i32 { let a = Vec2 { x: 2, y: 3 }; let b = Vec2 { x: 5, y: 7 }; let c = a + b; return c.x; }";
    check_src(src).expect("overload block must check");
}

#[test]
fn r3_bare_overload_block_is_rejected() {
    // Corpus test_error_bare_overload: `overload` without `@`
    // is not a declaration.
    let src = "struct Vec2 { x: i32, y: i32, }\noverload Vec2 { fn +(self: Vec2, other: Vec2) -> Vec2 { return self; } }\nfn main() -> i32 { return 0; }";
    assert!(check_src(src).is_err(), "bare overload must be rejected");
}

#[test]
fn r3_mixed_plain_extern_variadic_below_floor() {
    // Corpus test_error_overload_variadic_too_few: a call below
    // every member's arity (plain member takes 2, the variadic
    // extern's fixed floor is 1) matches no member.
    let src = "fn tf(code: i32, extra: i32) -> i64 { return 0; }\nextern \"C\" fn tf(tag: ptr, args: ...) -> i64 { return 0; }\nfn main() -> i32 { let r = unsafe { tf() }; return r as i32; }";
    let err = check_src(src).expect_err("below-floor call must fail");
    assert!(err.1.contains("no overload of `tf` matches"), "got: {}", err.1);
}

#[test]
fn r3_mixed_plain_extern_variadic_resolves() {
    // Corpus test_unit_overload_variadic_arity (core shape): the
    // variadic member accepts at/above its fixed floor, the plain
    // member by its exact signature.
    let src = "fn vsum(code: i32, extra: i32) -> i64 { return 1000; }\nextern \"C\" fn vsum(tag: ptr, args: ...) -> i64 { return 7; }\nfn main() -> i32 { let a = unsafe { vsum(0, 5) }; let b = unsafe { vsum(1, 2) }; return (a + b) as i32; }";
    check_src(src).expect("mixed variadic set must resolve");
}

#[test]
fn r3_named_variadic_param_parses() {
    // The `args: ...` named va_list parameter (va_next_* body
    // surface is a std-call concern; the declaration must parse
    // and register as variadic).
    let src = "extern \"C\" fn vsum(tag: ptr, args: ...) -> i64 { return 0; }\nfn main() -> i32 { return 0; }";
    check_src(src).expect("named variadic declaration must check");
}

#[test]
fn r3_template_literal_checks_via_format_desugar() {
    // Corpus twin: demos/basic/94_nested_tpl — a template with
    // interpolations checks as the format() call it desugars to
    // (format/str_eq arrive through the std-import prescan exactly
    // as in the corpus file's environment; here the signatures are
    // declared inline with the same shapes).
    let src = "fn format(f: String, a: i32, b: i32) -> String { return f; }\nfn str_eq(a: String, b: String) -> bool { return true; }\nfn main() -> i32 { let x = 3; let y = 4; let s = `sum=${x + y} prod=${x * y}`; if !str_eq(s, \"sum=7 prod=12\") { return 1; }; return 0; }";
    check_src(src).expect("template literal must check via its format desugar");
}

#[test]
fn r3_defer_iife_callee_is_desugar_failed() {
    // Corpus neg_desugar_failed: defer of an immediately-invoked
    // closure literal — the desugarer cannot clone the callee.
    let src = "fn main() -> i32 { defer (|x: i32| x + 1)(3); return 0; }";
    let err = check_src(src).expect_err("defer IIFE must be rejected");
    assert_eq!(err.0, ErrorKind::DesugarFailed, "got: {}", err.1);
}

#[test]
fn r3_defer_plain_call_still_checks() {
    // Control: an ordinary named-call defer stays accepted.
    let src = "fn cleanup() -> i32 { return 0; }\nfn main() -> i32 { defer cleanup(); return 0; }";
    check_src(src).expect("plain defer must check");
}
