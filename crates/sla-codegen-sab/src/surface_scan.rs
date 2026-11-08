//! D5 (scan half) — future/thread usage scans and
//! `stackAllocSize`, split from `surface_preload.rs` for the
//! 700-line discipline (one decomposition unit, D5).
//!
//! Mirrors `sab_codegen.zig`: `isFutureTaskRuntimeCall`
//! (line 2107), `nodeUsesFutureTaskRuntime` (2112–2245),
//! `blockUsesFutureTaskRuntime`, `programUsesFutureTaskRuntime`
//! (2247–2253), and `stackAllocSize` (2419). The runtime-call
//! plans (`planTaskRuntimeCall` / `planExecutorRuntimeCall`,
//! lowering_rules.zig:3050–3072) are a deferred family in
//! sla-lowering; the name tables are ported locally here, as
//! in D3, and marked for re-homing.

use sla_ast::{CallExpr, Node};
use sla_lowering::int_constant_expr_value;

use crate::error::{Error, Result};

/// Zig `planTaskRuntimeCall` presence (lowering_rules.zig:
/// 3050–3060): `task::{new,poll,is_ready,result,state}`.
fn plan_task_runtime_call(call: &CallExpr) -> bool {
    call.associated_target.as_deref() == Some("task")
        && matches!(
            call.func_name.as_str(),
            "new" | "poll" | "is_ready" | "result" | "state"
        )
}

/// Zig `planExecutorRuntimeCall` presence (lowering_rules.zig:
/// 3061–3072): `executor::{new,poll_one,poll_ready_count}`.
fn plan_executor_runtime_call(call: &CallExpr) -> bool {
    call.associated_target.as_deref() == Some("executor")
        && matches!(
            call.func_name.as_str(),
            "new" | "poll_one" | "poll_ready_count"
        )
}

/// Zig `isFutureTaskRuntimeCall` (line 2107).
pub fn is_future_task_runtime_call(call: &CallExpr) -> bool {
    plan_task_runtime_call(call) || plan_executor_runtime_call(call)
}

/// Zig `nodeUsesFutureTaskRuntime` (lines 2112–2245): the full
/// scan behind `programUsesFutureTaskRuntime`. Any `await` and
/// any async function counts, per the SA-text parity comment in
/// the original. Node kinds without an arm answer false.
pub fn node_uses_future_task_runtime(node: &Node) -> bool {
    match node {
        Node::FuncDecl(f) => f.is_async || block_uses_future_task_runtime(&f.body),
        Node::TestDecl(t) => block_uses_future_task_runtime(&t.body),
        Node::LetStmt(l) => node_uses_future_task_runtime(&l.value),
        Node::LetDestructureStmt(l) => node_uses_future_task_runtime(&l.value),
        Node::LetElseStmt(l) => {
            node_uses_future_task_runtime(&l.value)
                || block_uses_future_task_runtime(&l.else_block)
        }
        Node::ConstStmt(c) => node_uses_future_task_runtime(&c.value),
        Node::AssignStmt(a) => {
            node_uses_future_task_runtime(&a.target) || node_uses_future_task_runtime(&a.value)
        }
        Node::ExprStmt(e) => node_uses_future_task_runtime(e),
        Node::ReturnStmt(r) => r
            .value
            .as_ref()
            .map(|v| node_uses_future_task_runtime(v))
            .unwrap_or(false),
        Node::BlockStmt(b) => block_uses_future_task_runtime(&b.body),
        Node::ArenaStmt(b) => block_uses_future_task_runtime(&b.body),
        Node::AwaitExpr(_) => true,
        Node::BinaryExpr(b) => {
            node_uses_future_task_runtime(&b.left) || node_uses_future_task_runtime(&b.right)
        }
        Node::BorrowExpr(b) => node_uses_future_task_runtime(&b.expr),
        Node::MoveExpr(m) => node_uses_future_task_runtime(&m.expr),
        Node::DerefExpr(d) => node_uses_future_task_runtime(&d.expr),
        Node::CastExpr(c) => node_uses_future_task_runtime(&c.expr),
        Node::FieldExpr(f) => node_uses_future_task_runtime(&f.expr),
        Node::TryExpr(t) => node_uses_future_task_runtime(&t.expr),
        Node::UnsafeExpr(u) => block_uses_future_task_runtime(&u.body),
        Node::StructLiteral(l) => {
            l.fields.iter().any(|f| node_uses_future_task_runtime(&f.value))
                || l.update_expr
                    .as_ref()
                    .map(|u| node_uses_future_task_runtime(u))
                    .unwrap_or(false)
        }
        Node::TupleLiteral(l) => l
            .elements
            .iter()
            .any(|e| node_uses_future_task_runtime(e)),
        Node::ArrayLiteral(l) => l
            .elements
            .iter()
            .any(|e| node_uses_future_task_runtime(e)),
        Node::EnumLiteral(l) => l
            .fields
            .iter()
            .any(|f| node_uses_future_task_runtime(&f.value)),
        Node::RepeatArrayLiteral(l) => node_uses_future_task_runtime(&l.value),
        Node::ClosureLiteral(l) => node_uses_future_task_runtime(&l.body),
        Node::CallExpr(call) => {
            if is_future_task_runtime_call(call) {
                return true;
            }
            if let Some(callee) = &call.callee {
                if node_uses_future_task_runtime(callee) {
                    return true;
                }
            }
            call.args.iter().any(|a| node_uses_future_task_runtime(a))
        }
        Node::IndexExpr(i) => {
            node_uses_future_task_runtime(&i.target) || node_uses_future_task_runtime(&i.index)
        }
        Node::SliceExpr(s) => {
            node_uses_future_task_runtime(&s.target)
                || node_uses_future_task_runtime(&s.start)
                || node_uses_future_task_runtime(&s.end)
        }
        Node::IfExpr(i) => {
            if node_uses_future_task_runtime(&i.cond) {
                return true;
            }
            if let Some(chain) = &i.let_chain {
                if chain.iter().any(|c| node_uses_future_task_runtime(&c.value)) {
                    return true;
                }
            }
            if block_uses_future_task_runtime(&i.then_block) {
                return true;
            }
            i.else_block
                .as_ref()
                .map(|b| block_uses_future_task_runtime(b))
                .unwrap_or(false)
        }
        Node::SwitchExpr(s) => {
            if node_uses_future_task_runtime(&s.val) {
                return true;
            }
            s.cases.iter().any(|case| {
                node_uses_future_task_runtime(&case.pattern)
                    || block_uses_future_task_runtime(&case.body)
            })
        }
        Node::MatchExpr(m) => {
            if node_uses_future_task_runtime(&m.val) {
                return true;
            }
            // Zig also walks each case's pattern node; the Rust
            // AST's match pattern is an EnumPattern (no embedded
            // expression), so only guard + body remain.
            m.cases.iter().any(|case| {
                case.guard
                    .as_ref()
                    .map(|g| node_uses_future_task_runtime(g))
                    .unwrap_or(false)
                    || block_uses_future_task_runtime(&case.body)
            })
        }
        Node::WhileStmt(w) => {
            node_uses_future_task_runtime(&w.cond) || block_uses_future_task_runtime(&w.body)
        }
        Node::ForStmt(f) => {
            node_uses_future_task_runtime(&f.start)
                || f.end
                    .as_ref()
                    .map(|e| node_uses_future_task_runtime(e))
                    .unwrap_or(false)
                || block_uses_future_task_runtime(&f.body)
        }
        Node::ImplDecl(i) => i
            .methods
            .iter()
            .any(|m| node_uses_future_task_runtime(m)),
        Node::OverloadDecl(o) => o
            .methods
            .iter()
            .any(|m| node_uses_future_task_runtime(m)),
        _ => false,
    }
}

/// Zig `blockUsesFutureTaskRuntime`.
pub fn block_uses_future_task_runtime(body: &[Node]) -> bool {
    body.iter().any(|stmt| node_uses_future_task_runtime(stmt))
}

/// Zig `programUsesFutureTaskRuntime` (lines 2247–2253).
pub fn program_uses_future_task_runtime(program: &Node) -> bool {
    match program {
        Node::Program(p) => p
            .decls
            .iter()
            .any(|decl| node_uses_future_task_runtime(decl)),
        _ => false,
    }
}

/// Zig `stackAllocSize` (line 2419): the first argument's
/// constant value when it folds, else the 16-byte default; a
/// negative constant is loud. `scalar_consts` is the C1 table
/// (name → the literal node it folded to).
pub fn stack_alloc_size(
    call: &CallExpr,
    scalar_consts: &std::collections::HashMap<String, Node>,
) -> Result<usize> {
    if !call.args.is_empty() {
        let resolve = |name: &str| -> Option<i64> {
            match scalar_consts.get(name) {
                Some(Node::Literal(sla_ast::Literal::IntVal(v))) => Some(*v),
                _ => None,
            }
        };
        let value = match int_constant_expr_value(&call.args[0], &resolve) {
            Some(value) => value,
            None => return Ok(16),
        };
        if value < 0 {
            return Err(Error::UnsupportedSabDirectFeature(
                "stack_alloc with a negative constant size [group D5 surface_preload; Zig sab_codegen.zig:2419-2427]"
                    .to_string(),
            ));
        }
        return Ok(value as usize);
    }
    Ok(16)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sla_ast::{AwaitExpr, FuncDecl, Literal, Primitive, Program, Type};
    use std::collections::HashMap;

    fn call(name: &str, target: Option<&str>, args: Vec<Node>) -> Node {
        Node::CallExpr(CallExpr {
            func_name: name.to_string(),
            associated_target: target.map(|t| t.to_string()),
            generics: vec![],
            args,
            callee: None,
        })
    }

    fn func(name: &str, body: Vec<Node>) -> Node {
        Node::FuncDecl(FuncDecl {
            is_variadic: false,
            name: name.to_string(),
            is_pub: false,
            is_extern: false,
            abi: None,
            no_mangle: false,
            is_decl_only: false,
            generics: vec![],
            params: vec![],
            ret_ty: Box::new(Type::Primitive(Primitive::VoidType)),
            body,
            is_inline: false,
            is_async: false,
            operator: None,
            is_arena: false,
            ret_fallible_sugar: false,
            ret_borrow_origin: None,
        })
    }

    fn as_call(node: Node) -> CallExpr {
        match node {
            Node::CallExpr(c) => c,
            _ => unreachable!(),
        }
    }

    #[test]
    fn future_scan_arms() {
        // Async fn counts even with an empty body.
        let mut f = func("main", vec![]);
        if let Node::FuncDecl(fd) = &mut f {
            fd.is_async = true;
        }
        let program = Node::Program(Program { decls: vec![f] });
        assert!(program_uses_future_task_runtime(&program));
        // A task::new call counts.
        let program2 = Node::Program(Program {
            decls: vec![func(
                "main",
                vec![Node::ExprStmt(Box::new(call("new", Some("task"), vec![])))],
            )],
        });
        assert!(program_uses_future_task_runtime(&program2));
        // An await anywhere counts.
        let program3 = Node::Program(Program {
            decls: vec![func(
                "main",
                vec![Node::ExprStmt(Box::new(Node::AwaitExpr(AwaitExpr {
                    expr: Box::new(Node::Identifier("f".into())),
                })))],
            )],
        });
        assert!(program_uses_future_task_runtime(&program3));
        // A plain program does not.
        let program4 = Node::Program(Program {
            decls: vec![func(
                "main",
                vec![Node::ExprStmt(Box::new(call("helper", None, vec![])))],
            )],
        });
        assert!(!program_uses_future_task_runtime(&program4));
        // thread::spawn is NOT a future-task call in Zig's scan.
        assert!(!is_future_task_runtime_call(&as_call(call(
            "spawn",
            Some("thread"),
            vec![Node::Identifier("j".into())]
        ))));
        assert!(is_future_task_runtime_call(&as_call(call(
            "poll_one",
            Some("executor"),
            vec![]
        ))));
    }

    #[test]
    fn stack_alloc_sizes() {
        let consts: HashMap<String, Node> = HashMap::new();
        let bare = as_call(call("stack_alloc", None, vec![]));
        assert_eq!(stack_alloc_size(&bare, &consts).unwrap(), 16);
        let lit = as_call(call(
            "stack_alloc",
            None,
            vec![Node::Literal(Literal::IntVal(64))],
        ));
        assert_eq!(stack_alloc_size(&lit, &consts).unwrap(), 64);
        let neg = as_call(call(
            "stack_alloc",
            None,
            vec![Node::Literal(Literal::IntVal(-1))],
        ));
        assert!(stack_alloc_size(&neg, &consts).is_err());
        // Non-constant argument falls back to 16, as in Zig.
        let var = as_call(call(
            "stack_alloc",
            None,
            vec![Node::Identifier("n".into())],
        ));
        assert_eq!(stack_alloc_size(&var, &consts).unwrap(), 16);
        // A scalar-const identifier resolves through the C1 table.
        let mut consts2: HashMap<String, Node> = HashMap::new();
        consts2.insert("N".to_string(), Node::Literal(Literal::IntVal(48)));
        let by_name = as_call(call(
            "stack_alloc",
            None,
            vec![Node::Identifier("N".into())],
        ));
        assert_eq!(stack_alloc_size(&by_name, &consts2).unwrap(), 48);
    }
}
