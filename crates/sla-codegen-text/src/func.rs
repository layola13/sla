//! Function and test declaration emission, mirrored from
//! codegen.zig's `genFuncDeclNamed` (7429-7606) and `genTestDecl`
//! (7610-7700). The async continuation entry points checked at the
//! top of `genFuncDeclNamed` are deferred (lowering async family), so
//! an async function fails loudly here.

use sla_ast::{FuncDecl, Node, TestDecl, Type};
use sla_lowering::{
    abi_param_type_string, block_terminates, by_value_raw_pointer_param, is_borrow_like_type,
    is_void_type, shared_abi_param_prefix,
};

use crate::codegen::{sanitize_test_name, type_size, type_string, Codegen};
use crate::{unsupported, Result};

impl<'c> Codegen<'c> {
    /// codegen.zig:7608 `genFuncDecl`.
    pub(crate) fn gen_func_decl(&mut self, f: &FuncDecl) -> Result<()> {
        let name = f.name.clone();
        self.gen_func_decl_named(&name, f)
    }

    /// codegen.zig:7429 `genFuncDeclNamed`.
    pub(crate) fn gen_func_decl_named(&mut self, name: &str, f: &FuncDecl) -> Result<()> {
        if f.is_async {
            return unsupported(
                "async function (continuation plans deferred)",
                "lowering_rules.zig planAsync*Continuation family",
            );
        }
        self.reset_function_state();
        self.repeated_let_bindings = sla_lowering::collect_repeated_let_bindings(&f.body);
        self.collect_assigned_bindings(&f.body);
        self.collect_addressable_bindings(&f.body);

        // Signature.
        let lowered_name = self.lowered_func_symbol(name);
        let mut sig = format!("@{lowered_name}(");
        for (i, p) in f.params.iter().enumerate() {
            if i > 0 {
                sig.push_str(", ");
            }
            let prefix = shared_abi_param_prefix(
                p.is_borrow,
                p.is_move,
                matches!(&*p.ty, Type::Borrow(_)),
                by_value_raw_pointer_param(p),
                self.type_is_copy_value(&p.ty),
                is_borrow_like_type(&p.ty),
            );
            sig.push_str(&format!(
                "{prefix}{}: {}",
                p.name,
                abi_param_type_string(p.is_borrow, p.is_move, &p.ty)
            ));
        }
        if is_void_type(&f.ret_ty) {
            sig.push_str("):");
        } else {
            sig.push_str(&format!(
                ") -> {}:",
                sla_lowering::abi_return_type_string(&f.ret_ty)
            ));
        }
        self.wl(&sig);
        self.wl("L_ENTRY:");

        // Hoisted loop allocations + for-loop counter slots.
        self.emit_hoisted_and_counter_slots(&f.body)?;

        // Parameter prologues.
        for p in &f.params {
            self.remember_local_binding_type(&p.name, &p.ty);
            if let Type::Borrow(inner) = &*p.ty {
                if matches!(&**inner, Type::Array(_)) {
                    return unsupported(
                        "borrowed slice parameter prologue",
                        "codegen.zig:7540 (slice param materialization)",
                    );
                }
            }
            if !p.is_borrow && !p.is_move && self.binding_needs_addressable_storage(&p.name, &p.ty)
            {
                let raw_param = self.new_tmp();
                self.ins(&format!("{raw_param} = {}", p.name));
                self.stack_alloc_bindings.insert(p.name.clone());
                self.ins(&format!(
                    "{} = stack_alloc {}",
                    p.name,
                    type_size(&p.ty)
                ));
                self.ins(&format!(
                    "store {}+0, {raw_param} as {}",
                    p.name,
                    type_string(&p.ty)
                ));
                self.emit_release(&raw_param)?;
            } else if !p.is_borrow
                && !p.is_move
                && self.binding_needs_assigned_value_slot(&p.name, &p.ty)
            {
                let raw_param = self.new_tmp();
                self.ins(&format!("{raw_param} = {}", p.name));
                self.stack_alloc_bindings.insert(p.name.clone());
                self.assigned_value_slots.insert(p.name.clone());
                self.ins(&format!(
                    "{} = stack_alloc {}",
                    p.name,
                    type_size(&p.ty)
                ));
                self.ins(&format!(
                    "store {}+0, {raw_param} as {}",
                    p.name,
                    type_string(&p.ty)
                ));
            }
        }

        // Body: tail-expression return path or plain block.
        let tail_expr_return = !is_void_type(&f.ret_ty)
            && !f.body.is_empty()
            && matches!(f.body.last(), Some(Node::ExprStmt(_)))
            && !block_terminates(&f.body);
        if tail_expr_return {
            let split = f.body.len() - 1;
            for stmt in &f.body[..split] {
                self.gen_stmt(stmt)?;
            }
            let last = &f.body[split];
            let tail_expr = match last {
                Node::ExprStmt(expr) => expr,
                _ => unreachable!(),
            };
            let tail_reg = self.gen_expr(tail_expr)?;
            self.emit_function_tail_cleanups(last, tail_expr)?;
            self.ins(&format!("return {tail_reg}"));
        } else {
            self.gen_block(&f.body)?;
        }

        if is_void_type(&f.ret_ty) && !last_stmt_is_return(&f.body) {
            self.ins("return");
        }
        self.w("\n");
        Ok(())
    }

    /// codegen.zig:7610 `genTestDecl`. The body is generated exactly
    /// as given; build-mode test-body pruning happens upstream of
    /// codegen in the Zig pipeline (the build artifacts carry empty
    /// bodies), so a pruned program emits the stub shape seen in the
    /// goldens from this same code path.
    pub(crate) fn gen_test_decl(&mut self, t: &TestDecl) -> Result<()> {
        self.reset_function_state();
        self.repeated_let_bindings = sla_lowering::collect_repeated_let_bindings(&t.body);
        self.collect_assigned_bindings(&t.body);
        self.collect_addressable_bindings(&t.body);

        let mut header = String::from("@test");
        if t.is_ignored {
            header.push_str(" ignored");
        }
        if t.should_panic {
            header.push_str(" should_panic");
        }
        header.push_str(&format!(" \"{}\"():", sanitize_test_name(&t.name)));
        self.wl(&header);
        let entry_label = self.new_label("L_TEST_ENTRY");
        self.wl(&format!("{entry_label}:"));

        self.emit_hoisted_and_counter_slots(&t.body)?;
        self.gen_block(&t.body)?;
        self.ins("return");
        self.w("\n");
        Ok(())
    }

    /// Hoisted stack allocations (`stack_alloc 16`) and for-loop
    /// counter slots (`stack_alloc 8`), shared by func/test decls.
    fn emit_hoisted_and_counter_slots(&mut self, body: &[Node]) -> Result<()> {
        let mut hoisted: Vec<String> = Vec::new();
        self.collect_hoisted_allocs(body, &mut hoisted);
        self.hoisted_allocs = hoisted.clone();
        for h_name in &hoisted {
            self.ins(&format!("{h_name} = stack_alloc 16"));
        }
        let mut slots: Vec<String> = Vec::new();
        self.collect_loop_counter_slots(body, &mut slots);
        for slot_name in &slots {
            self.ins(&format!("{slot_name} = stack_alloc 8"));
            self.stack_alloc_bindings.insert(slot_name.clone());
        }
        Ok(())
    }

    /// codegen.zig:2553 `bindingNeedsAddressableStorage`.
    pub(crate) fn binding_needs_addressable_storage(&self, name: &str, ty: &Type) -> bool {
        sla_lowering::plan_borrowed_binding_storage(self.addressable_bindings.contains(name), ty)
            .materialize_stack_slot
    }

    /// codegen.zig:2557 `bindingNeedsAssignedValueSlot`.
    pub(crate) fn binding_needs_assigned_value_slot(&self, name: &str, ty: &Type) -> bool {
        self.assigned_bindings.contains(name)
            && !self.type_is_copy_value(ty)
            && self.check.type_is_shallow_copy_call_arg_value(ty)
    }
}

/// codegen.zig `lastStmtIsReturn`.
fn last_stmt_is_return(body: &[Node]) -> bool {
    matches!(body.last(), Some(Node::ReturnStmt(_)))
}
