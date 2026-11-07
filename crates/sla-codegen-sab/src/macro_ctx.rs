//! G group: macro-expansion context mechanics.
//!
//! Faithful port of the `MacroExpansionContext` helper family in
//! `sab_codegen.zig` (reference tree @3de91932):
//! - `macroArgBinding` (8867), `macroArg` (8873),
//!   `macroLocal` (8879), `macroIdentifierName` (8885),
//!   `macroIdentifierType` (8891)
//! - `macroScopeMark` (8893), `restoreMacroLocals` (8897)
//! - `newMacroLocalName` (8908), `putMacroLocal` (8916),
//!   `defineMacroLocal` (8926)
//! - `macroEffectiveArg` (9584)
//!
//! The context structure itself is B-group's `types.rs`
//! (`MacroExpansionContext`); this module is the operations over
//! it. Zig's `catch unreachable` in `restoreMacroLocals` (the
//! `locals.put` on restore) has no failure mode in this port:
//! restoring into a `HashMap` cannot fail. The hygiene name format
//! `__sla_macro_{macro}_{invocation}_{idx}_{name}` is reproduced
//! exactly, including the per-context monotonically increasing
//! `local_idx` (never rewound by scope restores — Zig restores the
//! bindings, not the counter).

use sla_ast::{Node, Type};

use crate::types::{MacroArgBinding, MacroExpansionContext, MacroLocalBinding};

/// Zig `macroArgBinding` (sab_codegen.zig:8867): first binding with
/// this name, in argument order.
pub fn arg_binding(ctx: &MacroExpansionContext, name: &str) -> Option<MacroArgBinding> {
    ctx.args.iter().find(|b| b.name == name).cloned()
}

/// Zig `macroArg` (sab_codegen.zig:8873).
pub fn arg(ctx: &MacroExpansionContext, name: &str) -> Option<Node> {
    arg_binding(ctx, name).map(|b| b.arg)
}

/// Zig `macroLocal` (sab_codegen.zig:8879).
pub fn local(ctx: &MacroExpansionContext, name: &str) -> Option<MacroLocalBinding> {
    ctx.locals.get(name).cloned()
}

/// Zig `macroIdentifierName` (sab_codegen.zig:8885): the hygienic
/// mapped name for a macro-local identifier, if any.
pub fn identifier_name(ctx: &MacroExpansionContext, name: &str) -> Option<String> {
    local(ctx, name).map(|l| l.mapped)
}

/// Zig `macroIdentifierType` (sab_codegen.zig:8891).
pub fn identifier_type(ctx: &MacroExpansionContext, name: &str) -> Option<Type> {
    local(ctx, name).and_then(|l| l.ty)
}

/// Zig `macroScopeMark` (sab_codegen.zig:8893).
pub fn scope_mark(ctx: &MacroExpansionContext) -> usize {
    ctx.local_changes.len()
}

/// Zig `restoreMacroLocals` (sab_codegen.zig:8897): pop recorded
/// changes back to `mark`, restoring overwritten bindings and
/// removing freshly introduced ones.
pub fn restore_locals(ctx: &mut MacroExpansionContext, mark: usize) {
    while ctx.local_changes.len() > mark {
        let change = ctx.local_changes.pop().expect("len checked");
        if change.had_old {
            if let Some(old) = change.old {
                ctx.locals.insert(change.name.clone(), old);
            }
        } else {
            ctx.locals.remove(&change.name);
        }
    }
}

/// Zig `newMacroLocalName` (sab_codegen.zig:8908): allocate the
/// next hygienic name for a macro local. The name is recorded in
/// `allocated_names`, exactly as Zig appends it.
pub fn new_local_name(ctx: &mut MacroExpansionContext, name: &str) -> String {
    let idx = ctx.local_idx;
    ctx.local_idx += 1;
    let mapped = format!(
        "__sla_macro_{}_{}_{}_{}",
        ctx.macro_name, ctx.invocation, idx, name
    );
    ctx.allocated_names.push(mapped.clone());
    mapped
}

/// Zig `putMacroLocal` (sab_codegen.zig:8916): bind `name` to
/// `mapped`, recording the displaced binding (if any) on the
/// change stack for `restore_locals`.
pub fn put_local(
    ctx: &mut MacroExpansionContext,
    name: &str,
    mapped: &str,
    ty: Option<Type>,
) {
    let old = ctx.locals.get(name).cloned();
    ctx.local_changes.push(crate::types::MacroLocalChange {
        name: name.to_string(),
        had_old: old.is_some(),
        old,
    });
    ctx.locals.insert(
        name.to_string(),
        MacroLocalBinding {
            mapped: mapped.to_string(),
            ty,
        },
    );
}

/// Zig `defineMacroLocal` (sab_codegen.zig:8926): allocate a
/// hygienic name and bind it in one step.
pub fn define_local(ctx: &mut MacroExpansionContext, name: &str, ty: Option<Type>) -> String {
    let mapped = new_local_name(ctx, name);
    put_local(ctx, name, &mapped, ty);
    mapped
}

/// Zig `macroEffectiveArg` (sab_codegen.zig:9584): resolve an
/// argument expression through macro-local shadowing and nested
/// argument bindings. An identifier that names a macro *local* is
/// not substituted; an identifier that names a macro *argument*
/// is replaced by the (recursively resolved) caller-side argument
/// expression.
pub fn effective_arg(ctx: &MacroExpansionContext, arg: &Node) -> Node {
    if let Node::Identifier(name) = arg {
        if local(ctx, name).is_none() {
            if let Some(binding) = arg_binding(ctx, name) {
                if let Some(arg_ctx) = &binding.ctx {
                    let inner = arg_ctx.borrow();
                    return effective_arg(&inner, &binding.arg);
                }
                return binding.arg.clone();
            }
        }
    }
    arg.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sla_ast::Primitive;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;

    fn ctx() -> MacroExpansionContext {
        MacroExpansionContext {
            macro_name: "VEC_PUSH".to_string(),
            invocation: 3,
            local_idx: 0,
            args: vec![],
            locals: HashMap::new(),
            local_changes: vec![],
            allocated_names: vec![],
        }
    }

    fn binding(name: &str, arg: Node, ctx: Option<Rc<RefCell<MacroExpansionContext>>>) -> MacroArgBinding {
        MacroArgBinding {
            name: name.to_string(),
            arg,
            ctx,
            evaluated_reg: None,
            release_evaluated_reg: false,
        }
    }

    #[test]
    fn hygienic_names_use_invocation_and_counter() {
        let mut c = ctx();
        let a = define_local(&mut c, "tmp", None);
        let b = define_local(&mut c, "tmp", None);
        assert_eq!(a, "__sla_macro_VEC_PUSH_3_0_tmp");
        assert_eq!(b, "__sla_macro_VEC_PUSH_3_1_tmp");
        assert_eq!(c.allocated_names, vec![a.clone(), b.clone()]);
        // Second binding shadows the first for lookup.
        assert_eq!(identifier_name(&c, "tmp"), Some(b));
    }

    #[test]
    fn scope_restore_reverts_and_removes() {
        let mut c = ctx();
        define_local(&mut c, "x", Some(Type::Primitive(Primitive::I32)));
        let mark = scope_mark(&c);
        define_local(&mut c, "x", Some(Type::Primitive(Primitive::Boolean)));
        define_local(&mut c, "y", None);
        assert_eq!(
            identifier_type(&c, "x"),
            Some(Type::Primitive(Primitive::Boolean))
        );
        restore_locals(&mut c, mark);
        assert_eq!(
            identifier_type(&c, "x"),
            Some(Type::Primitive(Primitive::I32))
        );
        assert!(local(&c, "y").is_none());
        // The name counter is NOT rewound by a restore (Zig parity).
        assert_eq!(c.local_idx, 3);
    }

    #[test]
    fn arg_lookup_first_match_wins() {
        let mut c = ctx();
        c.args.push(binding("v", Node::Identifier("one".to_string()), None));
        c.args.push(binding("v", Node::Identifier("two".to_string()), None));
        assert_eq!(arg(&c, "v"), Some(Node::Identifier("one".to_string())));
        assert_eq!(arg(&c, "missing"), None);
    }

    #[test]
    fn effective_arg_substitutes_and_recurses() {
        // Outer ctx: param `p` bound to identifier `q` in inner ctx;
        // inner ctx: `q` bound to literal 7.
        let mut inner = ctx();
        inner.args.push(binding(
            "q",
            Node::Literal(sla_ast::Literal::IntVal(7)),
            None,
        ));
        let inner_handle = Rc::new(RefCell::new(inner));
        let mut outer = ctx();
        outer.args.push(binding(
            "p",
            Node::Identifier("q".to_string()),
            Some(inner_handle),
        ));
        let resolved = effective_arg(&outer, &Node::Identifier("p".to_string()));
        assert_eq!(resolved, Node::Literal(sla_ast::Literal::IntVal(7)));
    }

    #[test]
    fn effective_arg_local_shadowing_blocks_substitution() {
        let mut c = ctx();
        c.args.push(binding(
            "p",
            Node::Literal(sla_ast::Literal::IntVal(1)),
            None,
        ));
        define_local(&mut c, "p", None);
        // `p` is a macro local here: no substitution.
        let resolved = effective_arg(&c, &Node::Identifier("p".to_string()));
        assert_eq!(resolved, Node::Identifier("p".to_string()));
        // Non-identifier args pass through untouched.
        let lit = Node::Literal(sla_ast::Literal::IntVal(9));
        assert_eq!(effective_arg(&c, &lit), lit);
    }
}
