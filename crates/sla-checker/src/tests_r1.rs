//! R-1 regression tests: the std callable surface in ORDINARY
//! fn bodies. Census class R-1 (corpus/CENSUS.md): the CLI
//! front end assembled the std associated leaves, the
//! receiver-shape leaf cascade and the imported-macro registry
//! for NO checked body — `sla check` only appeared to accept
//! them inside @test bodies because the Check front end never
//! parses @test bodies at all (proven by the vacuous-pass
//! probes: an @test calling a fictional function, and an @test
//! with a blatant type error, both "passed" check pre-fix).
//!
//! Every positive test below failed pre-fix with
//! "Undefined call: {name}" (error.UndefinedVariable) and
//! passes post-fix; the negative tests pin that the leaves
//! keep Zig's gates (arity, element types, the unsafe gate,
//! the Ordering variant set).

use crate::checker::{Checker, ImportedMacroInfo};
use crate::diag::Diagnostic;
use sla_ast::node::Node;

fn check_source(source: &str) -> Result<(), Diagnostic> {
    let root = sla_parser::parse_program(source).expect("source must parse");
    let Node::Program(program) = &root else {
        panic!("parser root must be a Program node");
    };
    let mut checker = Checker::new();
    checker.check_program(program)
}

fn assert_ok(source: &str) {
    let result = check_source(source);
    assert!(result.is_ok(), "expected ok, got {result:?}");
}

fn assert_err(source: &str) {
    let result = check_source(source);
    assert!(result.is_err(), "expected err, got ok");
}

/// The census minimal repro (probe a_fn_vec): `Vec::new` and
/// `push` in an ordinary fn body.
#[test]
fn r1_vec_new_and_push_in_fn_body() {
    assert_ok(
        "fn main() -> i32 {\n    let v = Vec::new();\n    v.push(1);\n    v.push(2);\n    return (len(v) as i32);\n}\n",
    );
}

/// Vec pop/remove leaves.
#[test]
fn r1_vec_pop_remove() {
    assert_ok(
        "fn main() -> i32 {\n    let v = Vec::new();\n    v.push(7);\n    let a = v.pop().unwrap_or(0);\n    v.push(9);\n    let b = v.remove(0);\n    return a + b;\n}\n",
    );
}

/// HashMap::new + insert + get in an fn body, with the
/// unannotated (infer) form concretized by the first insert.
#[test]
fn r1_hashmap_insert_get() {
    assert_ok(
        "fn main() -> i32 {\n    let m = HashMap::new();\n    m.insert(1, 40);\n    m.insert(2, 2);\n    if m.get(1).is_some() { return 1; };\n    return 0;\n}\n",
    );
}

/// HashSet::new + insert + contains.
#[test]
fn r1_hashset_insert_contains() {
    assert_ok(
        "fn main() -> i32 {\n    let s = HashSet::new();\n    s.insert(3);\n    if s.contains(3) { return 1; };\n    return 0;\n}\n",
    );
}

/// future::ready / future::defer_ready associated leaves
/// (census first-error names: defer_ready x25, ready x11).
#[test]
fn r1_future_ready_defer_ready() {
    assert_ok(
        "fn main() -> i32 {\n    let f = future::ready(41);\n    let g = future::defer_ready(1);\n    return 0;\n}\n",
    );
}

/// Result unwrap_or on a call-result receiver
/// (census name unwrap_or x5).
#[test]
fn r1_result_unwrap_or_on_call_result() {
    assert_ok(
        "fn div(a: i32, b: i32) -> Result<i32, i32> {\n    if b == 0 { return Err(7); };\n    return Ok(a / b);\n}\nfn main() -> i32 {\n    let x = div(7, 2).unwrap_or(9);\n    return x;\n}\n",
    );
}

/// The rosetta-18 chain: Option map with an unannotated
/// closure parameter (context-filled) + unwrap_or.
#[test]
fn r1_option_map_closure_chain() {
    assert_ok(
        "fn main() -> i32 {\n    let value = Some(3).map(|x| x + 5).unwrap_or(0);\n    return value;\n}\n",
    );
}

/// Option query + unwrap_or_else with a nullary closure.
#[test]
fn r1_option_query_and_unwrap_or_else() {
    assert_ok(
        "fn main() -> i32 {\n    let some: Option<i32> = Some(5);\n    if !some.is_some() { return 1; };\n    let v = some.unwrap_or_else(|| 9);\n    return v;\n}\n",
    );
}

/// Array iter().sum() (census name iter x6; demos
/// rosetta/13_array_sum shape).
#[test]
fn r1_array_iter_sum() {
    assert_ok(
        "fn main() -> i32 {\n    let arr = [1, 2, 3, 4];\n    let total = arr.iter().sum();\n    return total;\n}\n",
    );
}

/// fold with a two-parameter context-filled closure.
#[test]
fn r1_vec_iter_fold() {
    assert_ok(
        "fn main() -> i32 {\n    let v = Vec::new();\n    v.push(1);\n    v.push(2);\n    let total = v.iter().fold(0, |acc, x| acc + x);\n    return total;\n}\n",
    );
}

/// AtomicI32::new + fetch_add + load with `Ordering::SeqCst`,
/// on a direct and on a dereferenced receiver (census name
/// fetch_add x3; the 123-barrier shape). Also pins the
/// `Ordering::Variant` identifier arm.
#[test]
fn r1_atomic_fetch_add_load() {
    assert_ok(
        "fn bump(p: *AtomicI32) -> i32 {\n    return (*p).fetch_add(1, Ordering::SeqCst);\n}\nfn main() -> i32 {\n    let a = AtomicI32::new(10);\n    let old = a.fetch_add(5, Ordering::SeqCst);\n    let cur = a.load(Ordering::SeqCst);\n    return old + cur;\n}\n",
    );
}

/// VecDeque::new + push_back + pop_front.
#[test]
fn r1_vecdeque_push_back_pop_front() {
    assert_ok(
        "fn main() -> i32 {\n    let d = VecDeque::new();\n    d.push_back(4);\n    let v = d.pop_front().unwrap_or(0);\n    return v;\n}\n",
    );
}

/// Rc::new + clone + strong_count (census name strong_count).
#[test]
fn r1_rc_new_clone_strong_count() {
    assert_ok(
        "fn main() -> i32 {\n    let r = Rc::new(5);\n    let r2 = r.clone();\n    let n = r.strong_count();\n    return (n as i32);\n}\n",
    );
}

/// Cell::new + set/get and RefCell::new + borrow.
#[test]
fn r1_cell_and_refcell() {
    assert_ok(
        "fn main() -> i32 {\n    let c = Cell::new(1);\n    c.set(41);\n    let cell = RefCell::new(7);\n    let b = cell.borrow();\n    return c.get() + 1;\n}\n",
    );
}

/// Mutex::new + lock.
#[test]
fn r1_mutex_new_lock() {
    assert_ok(
        "fn main() -> i32 {\n    let m = Mutex::new(0);\n    let g = m.lock();\n    if g.is_ok() { return 1; };\n    return 0;\n}\n",
    );
}

/// Box::new constructor (consumes a non-copy argument).
#[test]
fn r1_box_new() {
    assert_ok(
        "fn main() -> i32 {\n    let b = Box::new(42);\n    return 0;\n}\n",
    );
}

/// mpsc::channel + Sender::send + Receiver::recv.
#[test]
fn r1_mpsc_channel_send_recv() {
    assert_ok(
        "fn main() -> i32 {\n    let (tx, rx) = mpsc::channel();\n    tx.send(5);\n    let v = rx.recv().unwrap_or(0);\n    return v;\n}\n",
    );
}

/// Array as_ptr + pointer add chain (rosetta/112 shape; the
/// add leaf keys on the UNPEELED pointer tag).
#[test]
fn r1_array_as_ptr_add() {
    assert_ok(
        "fn main() -> i32 {\n    let data: [i32; 4] = [1, 2, 3, 4];\n    let p = data.as_ptr();\n    let q = p.add(2);\n    let third = unsafe { *data.as_ptr().add(2) };\n    return third;\n}\n",
    );
}

/// str::from_utf8 on a Slice<u8> parameter.
#[test]
fn r1_str_from_utf8() {
    assert_ok(
        "fn f(b: Slice<u8>) -> i32 {\n    let r = str::from_utf8(b);\n    if r.is_ok() { return 1; };\n    return 0;\n}\n",
    );
}

/// A registered imported macro resolves in an ordinary fn
/// body (the checker half of the R-1 macro class; the CLI
/// registration wiring is pinned by the census re-run —
/// STR_PTR x18 files first-error pre-fix).
#[test]
fn r1_imported_macro_registered_resolves() {
    let source = "fn main() -> i32 {\n    let s = \"hello\";\n    let p = STR_PTR(s);\n    return 0;\n}\n";
    let root = sla_parser::parse_program(source).expect("source must parse");
    let Node::Program(program) = &root else {
        panic!("parser root must be a Program node");
    };
    let mut checker = Checker::new();
    checker.register_imported_macro(
        "STR_PTR",
        ImportedMacroInfo {
            arity: 2,
            leading_outputs: 1,
            import_path: Some("sa_std/string.sa".to_string()),
            borrowed_arg_mask: 0,
            address_slot_arg_mask: 0,
            direct_callees: Vec::new(),
        },
    );
    let result = checker.check_program(program);
    assert!(result.is_ok(), "expected ok, got {result:?}");
}

// -- negative pins: the leaves keep Zig's gates ----------------------

/// Vec::new takes no arguments.
#[test]
fn r1_vec_new_with_arg_rejected() {
    assert_err("fn main() -> i32 {\n    let v = Vec::new(1);\n    return 0;\n}\n");
}

/// push of a mismatched element type is a TypeMismatch, not a
/// silent accept.
#[test]
fn r1_push_wrong_element_type_rejected() {
    assert_err(
        "fn main() -> i32 {\n    let v = Vec::new();\n    v.push(1);\n    v.push(\"text\");\n    return 0;\n}\n",
    );
}

/// fetch_add's second argument must be an Ordering value.
#[test]
fn r1_fetch_add_non_ordering_rejected() {
    assert_err(
        "fn main() -> i32 {\n    let a = AtomicI32::new(0);\n    let x = a.fetch_add(1, 2);\n    return x;\n}\n",
    );
}

/// An unknown `Ordering::Variant` is not an Ordering value.
#[test]
fn r1_ordering_unknown_variant_rejected() {
    assert_err(
        "fn main() -> i32 {\n    let a = AtomicI32::new(0);\n    let x = a.load(Ordering::Bogus);\n    return x;\n}\n",
    );
}

/// Box::from_raw stays behind the unsafe gate.
#[test]
fn r1_box_from_raw_outside_unsafe_rejected() {
    assert_err(
        "fn main() -> i32 {\n    let b = Box::new(1);\n    let p = Box::into_raw(b);\n    let b2 = Box::from_raw(p);\n    return 0;\n}\n",
    );
}

/// HashMap insert with a mismatched value type is rejected.
#[test]
fn r1_hashmap_insert_wrong_value_rejected() {
    assert_err(
        "fn main() -> i32 {\n    let m = HashMap::new();\n    m.insert(1, 40);\n    m.insert(2, \"text\");\n    return 0;\n}\n",
    );
}
