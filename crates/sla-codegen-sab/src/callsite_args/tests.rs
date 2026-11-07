use super::*;
use sla_ast::Primitive;
use std::collections::{HashMap, HashSet};

/// Scripted host: `gen_expr` interns `val{reg}` names in order,
/// types come from a fixed table keyed by debug shape, releases
/// and moves are recorded.
struct MockHost {
    builder: ModuleBuilder,
    next_reg: u32,
    types: HashMap<String, Type>,
    stack_locals: HashMap<String, u32>,
    emitted_slots: HashSet<u32>,
    locals: HashSet<u32>,
    releases: Vec<u32>,
    moves: Vec<u32>,
    consumed: Vec<u32>,
    address_source: AddressSource,
}

impl MockHost {
    fn new() -> Self {
        MockHost {
            builder: ModuleBuilder::new(),
            next_reg: 100,
            types: HashMap::new(),
            stack_locals: HashMap::new(),
            emitted_slots: HashSet::new(),
            locals: HashSet::new(),
            releases: vec![],
            moves: vec![],
            consumed: vec![],
            address_source: AddressSource {
                reg: 42,
                release_regs: vec![43],
                restore_slot: None,
            },
        }
    }
    fn key(arg: &Node) -> String {
        format!("{arg:?}")
    }
}

impl MacroCallHost for MockHost {
    fn builder(&mut self) -> &mut ModuleBuilder {
        &mut self.builder
    }
    fn gen_expr(&mut self, _arg: &Node, _ctx: Option<&MacroExpansionContext>) -> Result<u32> {
        let name = format!("val{}", self.next_reg);
        self.next_reg += 1;
        Ok(self.builder.intern(&name))
    }
    fn gen_address_of(
        &mut self,
        _arg: &Node,
        _ctx: Option<&MacroExpansionContext>,
    ) -> Result<AddressSource> {
        self.builder.intern("src42");
        Ok(self.address_source.clone())
    }
    fn arg_type(
        &mut self,
        arg: &Node,
        _ctx: Option<&MacroExpansionContext>,
    ) -> Result<Option<Type>> {
        Ok(self.types.get(&Self::key(arg)).cloned())
    }
    fn gen_string_literal(&mut self, _value: &str) -> Result<u32> {
        let reg = self.builder.intern("slice_lit");
        Ok(reg)
    }
    fn binding_reg(&mut self, name: &str) -> Result<u32> {
        Ok(self.builder.intern(&format!("bind_{name}")))
    }
    fn macro_assign_target_name(
        &mut self,
        _arg: &Node,
        _ctx: &MacroExpansionContext,
    ) -> Option<String> {
        None
    }
    fn stack_local_reg(&self, name: &str) -> Option<u32> {
        self.stack_locals.get(name).copied()
    }
    fn stack_alloc_emitted(&self, reg: u32) -> bool {
        self.emitted_slots.contains(&reg)
    }
    fn note_stack_alloc(&mut self, reg: u32) {
        self.emitted_slots.insert(reg);
    }
    fn is_local_reg(&self, reg: u32) -> bool {
        self.locals.contains(&reg)
    }
    fn has_global_scalar_const(&self, _name: &str) -> bool {
        false
    }
    fn expr_has_fnptr_type(&self, _arg: &Node) -> bool {
        false
    }
    fn funcs_contains(&self, _name: &str) -> bool {
        false
    }
    fn expr_type(&self, _arg: &Node) -> Option<Type> {
        None
    }
    fn has_constructor_surface_rule(&self, _t: &str, _n: &str) -> bool {
        false
    }
    fn type_is_pointer_scalar_value(&self, _ty: &Type) -> bool {
        false
    }
    fn emit_release(&mut self, reg: u32) -> Result<()> {
        self.releases.push(reg);
        Ok(())
    }
    fn emit_move(&mut self, reg: u32) -> Result<()> {
        self.moves.push(reg);
        Ok(())
    }
    fn mark_consumed(&mut self, reg: u32) -> Result<()> {
        self.consumed.push(reg);
        Ok(())
    }
}

fn ident(name: &str) -> Node {
    Node::Identifier(name.to_string())
}

fn plan(borrowed: u64, addr: u64, leading: usize, expr_out: bool) -> ImportedMacroCallPlan {
    ImportedMacroCallPlan {
        macro_name: "M".to_string(),
        import_path: Some("sa_std/m.sa".to_string()),
        arity: 2,
        leading_outputs: leading,
        borrowed_arg_mask: borrowed,
        address_slot_arg_mask: addr,
        expression_output: expr_out,
    }
}

#[test]
fn materialized_slot_arg_emits_alloc_store() {
    let mut host = MockHost::new();
    let arg = ident("x");
    host.types.insert(
        MockHost::key(&arg),
        Type::Primitive(Primitive::I32),
    );
    let lowered = gen_imported_macro_materialized_slot_arg(&mut host, &arg, None).unwrap();
    // tmp_0 slot; alloc size = i32 size (4); store of val100.
    assert_eq!(lowered.operand, "tmp_0");
    assert_eq!(lowered.release_reg, None);
    let module = host.builder.finish();
    let kinds: Vec<InstKind> = module.instructions.iter().map(|i| i.kind).collect();
    assert_eq!(kinds, vec![InstKind::StackAlloc, InstKind::Store]);
    assert_eq!(module.instructions[0].operands[1], Operand::ImmU64(4));
    assert_eq!(module.instructions[1].operands[2], Operand::Reg(0));
    assert_eq!(module.instructions[1].operands[3], Operand::Ty(PrimType::I32 as u32));
}

#[test]
fn fnptr_slot_loads_stores_and_moves() {
    let mut host = MockHost::new();
    let source = host.builder.intern("fp_slot");
    let lowered = materialize_fnptr_value_arg_slot(&mut host, source, true).unwrap();
    assert_eq!(lowered.operand, "tmp_1");
    assert_eq!(lowered.release_reg, Some(source));
    assert_eq!(host.moves.len(), 1);
    let module = host.builder.finish();
    let kinds: Vec<InstKind> = module.instructions.iter().map(|i| i.kind).collect();
    assert_eq!(
        kinds,
        vec![InstKind::Load, InstKind::StackAlloc, InstKind::Store]
    );
    assert_eq!(module.instructions[1].operands[1], Operand::ImmU64(8));
}

#[test]
fn address_expression_materialized_loads_then_stores() {
    let mut host = MockHost::new();
    let arg = Node::FieldExpr(sla_ast::FieldExpr {
        expr: Box::new(ident("s")),
        field_name: "f".to_string(),
    });
    host.types.insert(
        MockHost::key(&arg),
        Type::Primitive(Primitive::I64),
    );
    let lowered =
        gen_imported_macro_address_expression_materialized_slot_arg(&mut host, &arg, None)
            .unwrap();
    assert_eq!(lowered.operand, "tmp_0");
    // Cleanup releases the loaded temp (tmp_1 = reg 2) first —
    // field-expr results need release and it is not local —
    // then the non-local source reg 42 and its temp 43.
    assert_eq!(host.releases, vec![2, 42, 43]);
    let module = host.builder.finish();
    let kinds: Vec<InstKind> = module.instructions.iter().map(|i| i.kind).collect();
    assert_eq!(
        kinds,
        vec![InstKind::StackAlloc, InstKind::Load, InstKind::Store]
    );
}

#[test]
fn dispatch_btree_string_literal_special_case() {
    let mut host = MockHost::new();
    let mut p = plan(0b1, 0, 0, false);
    p.macro_name = "BTREE_GET".to_string();
    let arg = Node::Literal(Literal::StringVal("k".to_string()));
    host.types.insert(MockHost::key(&arg), Type::Infer);
    let lowered = gen_imported_macro_arg(&mut host, &p, 0, &arg, None).unwrap();
    assert_eq!(lowered.operand, "slice_lit");
}

#[test]
fn dispatch_leading_output_bind() {
    let mut host = MockHost::new();
    let p = plan(0, 0, 1, false);
    let arg = ident("out");
    host.types.insert(
        MockHost::key(&arg),
        Type::Primitive(Primitive::I32),
    );
    let lowered = gen_imported_macro_arg(&mut host, &p, 0, &arg, None).unwrap();
    assert_eq!(lowered.operand, "bind_out");
    assert_eq!(lowered.output_bind_name, Some("out".to_string()));
    assert!(lowered.output_store_slot.is_none());
}

#[test]
fn dispatch_reuse_existing_addressable() {
    let mut host = MockHost::new();
    let slot_reg = host.builder.intern("slot_x");
    host.stack_locals.insert("x".to_string(), slot_reg);
    let p = plan(0b1, 0, 0, false);
    let arg = ident("x");
    host.types.insert(
        MockHost::key(&arg),
        Type::Primitive(Primitive::I32),
    );
    let lowered = gen_imported_macro_arg(&mut host, &p, 0, &arg, None).unwrap();
    assert_eq!(lowered.operand, "slot_x");
    assert_eq!(lowered.release_reg, None);
}

#[test]
fn stack_alloc_reemit_is_noop() {
    let mut host = MockHost::new();
    let reg = host.builder.intern("shared_slot");
    emit_stack_alloc(&mut host, reg, 8);
    emit_stack_alloc(&mut host, reg, 8);
    let module = host.builder.finish();
    assert_eq!(module.instructions.len(), 1);
}
