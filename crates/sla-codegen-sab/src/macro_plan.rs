//! G group (ArgRole): imported-macro call plans and per-argument
//! lowering-role classification.
//!
//! Faithful port of the `lowering_rules.zig` imported-macro plan
//! family (reference tree `sa_plugin_sla` @3de91932):
//! - `ImportedMacroAddressableArgAction` (lowering_rules.zig:500)
//! - `ImportedMacroArgLoweringAction` (lowering_rules.zig:528)
//! - `ImportedMacroCallPlan` (lowering_rules.zig:570) and its
//!   classification methods (lines 570–672)
//! - `importedMacroDirectCallConsumesValueArg` (line 676)
//! - `AddressOfShape` / `planAddressOf` (lines 101, 5870) — only the
//!   shape computation `importedMacroArgAddressShape` consumes
//! - `abiPassesAsPointer` (line 3822),
//!   `importedMacroBorrowUsesRawPointerValue` (line 3833),
//!   `importedMacroArgUsesRawPointerValue` (line 3841)
//!
//! The plan *construction* (`planImportedMacroCall`,
//! lowering_rules.zig:3109) reads the checker's imported-macro
//! tables, which the Rust `CheckInfo` surface does not expose; the
//! plan therefore arrives here as an input value built by the
//! caller (see the G-group report's wiring requirements). Every
//! classification decision the SAB emitter makes from a plan is in
//! this file, byte-for-byte in decision structure.
//!
//! No `HashMap` iteration anywhere: classification is pure
//! per-argument computation over the plan's bit masks.

use sla_ast::{Node, Primitive, Type};

/// Zig `AddressOfShape` (lowering_rules.zig:101).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressOfShape {
    Identifier,
    DerefBorrowOrPointer,
    DerefSmartPointer,
    Field,
    Index,
    ValueTemp,
}

/// Zig `ImportedMacroAddressableArgAction` (lowering_rules.zig:500).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportedMacroAddressableArgAction {
    PassValue,
    PassAddressExpression,
    ReuseExistingAddressable,
    MaterializeStackSlot,
    MaterializeAddressExpressionStackSlot,
}

impl ImportedMacroAddressableArgAction {
    /// Zig `passesValue`.
    pub fn passes_value(self) -> bool {
        self == Self::PassValue
    }
    /// Zig `passesAddressExpression`.
    pub fn passes_address_expression(self) -> bool {
        self == Self::PassAddressExpression
    }
    /// Zig `reusesExistingAddressable`.
    pub fn reuses_existing_addressable(self) -> bool {
        self == Self::ReuseExistingAddressable
    }
    /// Zig `materializesStackSlot`.
    pub fn materializes_stack_slot(self) -> bool {
        self == Self::MaterializeStackSlot
    }
    /// Zig `materializesAddressExpressionStackSlot`.
    pub fn materializes_address_expression_stack_slot(self) -> bool {
        self == Self::MaterializeAddressExpressionStackSlot
    }
}

/// Zig `ImportedMacroArgLoweringAction` (lowering_rules.zig:528):
/// the ArgRole of one call-site argument in an imported-macro
/// expansion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportedMacroArgLoweringAction {
    PassValue,
    PassRawPointerValue,
    PassAddressExpression,
    PassPointerBackedProjection,
    ReuseExistingAddressable,
    MaterializeStackSlot,
    MaterializeAddressExpressionStackSlot,
}

impl ImportedMacroArgLoweringAction {
    /// Zig `passesValue`.
    pub fn passes_value(self) -> bool {
        self == Self::PassValue
    }
    /// Zig `passesRawPointerValue`.
    pub fn passes_raw_pointer_value(self) -> bool {
        self == Self::PassRawPointerValue
    }
    /// Zig `passesAddressExpression`.
    pub fn passes_address_expression(self) -> bool {
        self == Self::PassAddressExpression
    }
    /// Zig `passesPointerBackedProjection`.
    pub fn passes_pointer_backed_projection(self) -> bool {
        self == Self::PassPointerBackedProjection
    }
    /// Zig `reusesExistingAddressable`.
    pub fn reuses_existing_addressable(self) -> bool {
        self == Self::ReuseExistingAddressable
    }
    /// Zig `materializesStackSlot`.
    pub fn materializes_stack_slot(self) -> bool {
        self == Self::MaterializeStackSlot
    }
    /// Zig `materializesAddressExpressionStackSlot`.
    pub fn materializes_address_expression_stack_slot(self) -> bool {
        self == Self::MaterializeAddressExpressionStackSlot
    }
}

/// Zig `ImportedMacroCallPlan` (lowering_rules.zig:570).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedMacroCallPlan {
    pub macro_name: String,
    pub import_path: Option<String>,
    pub arity: usize,
    pub leading_outputs: usize,
    pub borrowed_arg_mask: u64,
    pub address_slot_arg_mask: u64,
    pub expression_output: bool,
}

impl ImportedMacroCallPlan {
    /// Zig `macroParamIndexForCallArg`.
    pub fn macro_param_index_for_call_arg(&self, call_arg_index: usize) -> usize {
        if self.expression_output {
            call_arg_index + self.leading_outputs
        } else {
            call_arg_index
        }
    }

    /// Zig `callArgNeedsAddressableSlot`.
    pub fn call_arg_needs_addressable_slot(&self, call_arg_index: usize) -> bool {
        let macro_idx = self.macro_param_index_for_call_arg(call_arg_index);
        if macro_idx >= 64 {
            return false;
        }
        let bit = 1u64 << macro_idx;
        (self.borrowed_arg_mask & bit) != 0 || (self.address_slot_arg_mask & bit) != 0
    }

    /// Zig `callArgNeedsDirectAddressSlot`.
    pub fn call_arg_needs_direct_address_slot(&self, call_arg_index: usize) -> bool {
        let macro_idx = self.macro_param_index_for_call_arg(call_arg_index);
        if macro_idx >= 64 {
            return false;
        }
        (self.address_slot_arg_mask & (1u64 << macro_idx)) != 0
    }

    /// Zig `addressableIdentifierArgName`.
    pub fn addressable_identifier_arg_name<'a>(
        &self,
        call_arg_index: usize,
        arg: &'a Node,
    ) -> Option<&'a str> {
        if !self.call_arg_needs_addressable_slot(call_arg_index) {
            return None;
        }
        match arg {
            Node::Identifier(name) => Some(name.as_str()),
            _ => None,
        }
    }

    /// Zig `planAddressableArgAction`.
    pub fn plan_addressable_arg_action(
        &self,
        call_arg_index: usize,
        has_existing_addressable_symbol: bool,
    ) -> ImportedMacroAddressableArgAction {
        self.plan_address_expression_arg_action(
            call_arg_index,
            AddressOfShape::Identifier,
            has_existing_addressable_symbol,
        )
    }

    /// Zig `planAddressExpressionArgAction`.
    pub fn plan_address_expression_arg_action(
        &self,
        call_arg_index: usize,
        address_shape: AddressOfShape,
        has_existing_addressable_symbol: bool,
    ) -> ImportedMacroAddressableArgAction {
        use ImportedMacroAddressableArgAction as A;
        if !self.call_arg_needs_addressable_slot(call_arg_index) {
            return A::PassValue;
        }
        if self.call_arg_needs_direct_address_slot(call_arg_index) {
            return match address_shape {
                AddressOfShape::Identifier => {
                    if has_existing_addressable_symbol {
                        A::ReuseExistingAddressable
                    } else {
                        A::MaterializeStackSlot
                    }
                }
                AddressOfShape::DerefBorrowOrPointer
                | AddressOfShape::DerefSmartPointer
                | AddressOfShape::Field
                | AddressOfShape::Index => A::PassAddressExpression,
                AddressOfShape::ValueTemp => A::PassValue,
            };
        }
        match address_shape {
            AddressOfShape::Identifier => {
                if has_existing_addressable_symbol {
                    A::ReuseExistingAddressable
                } else {
                    A::MaterializeStackSlot
                }
            }
            AddressOfShape::DerefBorrowOrPointer
            | AddressOfShape::DerefSmartPointer
            | AddressOfShape::Field
            | AddressOfShape::Index => A::MaterializeAddressExpressionStackSlot,
            AddressOfShape::ValueTemp => A::MaterializeStackSlot,
        }
    }

    /// Zig `planArgValueBypassAction`.
    pub fn plan_arg_value_bypass_action(
        &self,
        call_arg_index: usize,
        arg: &Node,
        arg_ty: &Type,
    ) -> Option<ImportedMacroArgLoweringAction> {
        use ImportedMacroArgLoweringAction as A;
        if !self.call_arg_needs_addressable_slot(call_arg_index) {
            return Some(A::PassValue);
        }
        if self.call_arg_needs_direct_address_slot(call_arg_index)
            && matches!(arg, Node::Identifier(_))
            && (matches!(arg_ty, Type::Infer)
                || abi_passes_as_pointer(arg_ty)
                || imported_macro_borrow_uses_raw_pointer_value(arg_ty))
        {
            return Some(A::PassValue);
        }
        if imported_macro_arg_uses_raw_pointer_value(arg, arg_ty) {
            return Some(A::PassRawPointerValue);
        }
        if matches!(arg, Node::Identifier(_)) && abi_passes_as_pointer(arg_ty) {
            return Some(A::PassValue);
        }
        None
    }

    /// Zig `planAddressableArgLoweringAction`.
    pub fn plan_addressable_arg_lowering_action(
        &self,
        call_arg_index: usize,
        address_shape: AddressOfShape,
        has_existing_addressable_symbol: bool,
        arg_ty: &Type,
    ) -> ImportedMacroArgLoweringAction {
        use ImportedMacroArgLoweringAction as A;
        if self.call_arg_needs_addressable_slot(call_arg_index)
            && !self.call_arg_needs_direct_address_slot(call_arg_index)
            && abi_passes_as_pointer(arg_ty)
            && (address_shape == AddressOfShape::Field || address_shape == AddressOfShape::Index)
        {
            return A::PassPointerBackedProjection;
        }
        if self.call_arg_needs_addressable_slot(call_arg_index)
            && !self.call_arg_needs_direct_address_slot(call_arg_index)
            && abi_passes_as_pointer(arg_ty)
            && (address_shape == AddressOfShape::DerefBorrowOrPointer
                || address_shape == AddressOfShape::DerefSmartPointer)
        {
            return A::PassAddressExpression;
        }
        match self.plan_address_expression_arg_action(
            call_arg_index,
            address_shape,
            has_existing_addressable_symbol,
        ) {
            ImportedMacroAddressableArgAction::PassValue => A::PassValue,
            ImportedMacroAddressableArgAction::PassAddressExpression => A::PassAddressExpression,
            ImportedMacroAddressableArgAction::ReuseExistingAddressable => {
                A::ReuseExistingAddressable
            }
            ImportedMacroAddressableArgAction::MaterializeStackSlot => A::MaterializeStackSlot,
            ImportedMacroAddressableArgAction::MaterializeAddressExpressionStackSlot => {
                A::MaterializeAddressExpressionStackSlot
            }
        }
    }
}

/// Zig `importedMacroDirectCallConsumesValueArg`
/// (lowering_rules.zig:676): imported macros whose body moves (`^`)
/// a by-value argument; the caller's binding is consumed by the
/// expansion, so no scope-end release may follow.
pub fn imported_macro_direct_call_consumes_value_arg(macro_name: &str, call_arg_index: usize) -> bool {
    (macro_name == "FS_READ_BUFFER_FREE" || macro_name == "SLA_FS_BUFFER_FREE") && call_arg_index == 0
}

/// Zig `abiPassesAsPointer` (lowering_rules.zig:3822).
pub fn abi_passes_as_pointer(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Pointer(_)
            | Type::Borrow(_)
            | Type::FnPtr(_)
            | Type::UserDefined(_)
            | Type::Tuple(_)
            | Type::Array(_)
            | Type::Future(_)
    )
}

/// Zig `importedMacroBorrowUsesRawPointerValue`
/// (lowering_rules.zig:3833).
pub fn imported_macro_borrow_uses_raw_pointer_value(arg_ty: &Type) -> bool {
    match arg_ty {
        Type::Primitive(p) => *p == Primitive::RawPtr,
        Type::Pointer(_) | Type::Borrow(_) => true,
        _ => false,
    }
}

/// Zig `importedMacroArgUsesRawPointerValue`
/// (lowering_rules.zig:3841).
pub fn imported_macro_arg_uses_raw_pointer_value(arg: &Node, arg_ty: &Type) -> bool {
    if imported_macro_borrow_uses_raw_pointer_value(arg_ty) {
        return true;
    }
    match arg {
        Node::CallExpr(call) => {
            call.func_name.ends_with("_PTR")
                || call.func_name.ends_with("_AS_PTR")
                || call.func_name == "as_ptr"
        }
        _ => false,
    }
}

/// Zig `userDefined` generic inner helper shape used across the
/// port: `Some` iff `ty` is `name` applied to exactly one generic.
fn user_defined_generic_inner<'a>(ty: &'a Type, name: &str) -> Option<&'a Type> {
    match ty {
        Type::UserDefined(ud) if ud.name == name && ud.generics.len() == 1 => {
            ud.generics.first()
        }
        _ => None,
    }
}

/// Zig `smartPointerDerefType` presence (lowering_rules.zig:4884):
/// Box/Rc/Arc are deref-able smart pointers; RefCell is not.
fn smart_pointer_deref_present(ty: &Type) -> bool {
    user_defined_generic_inner(ty, "Box").is_some()
        || user_defined_generic_inner(ty, "Rc").is_some()
        || user_defined_generic_inner(ty, "Arc").is_some()
}

/// Zig `plainBorrowOrPointerAddressSource` (lowering_rules.zig:118).
fn plain_borrow_or_pointer_address_source(ty: &Type) -> bool {
    match ty {
        Type::Borrow(_) | Type::Pointer(_) => {}
        _ => return false,
    }
    // Zig: `smartPointerType(ty) == null`. A borrow/pointer type is
    // never itself a user-defined smart pointer in this AST shape,
    // so the check is structurally always true here; kept explicit
    // to mirror the Zig predicate's two-step form.
    !(user_defined_generic_inner(ty, "Box").is_some()
        || user_defined_generic_inner(ty, "Rc").is_some()
        || user_defined_generic_inner(ty, "Arc").is_some()
        || user_defined_generic_inner(ty, "RefCell").is_some())
}

/// Zig `peelBorrowPointerType` (lowering_rules.zig:126).
fn peel_borrow_pointer_type(mut ty: &Type) -> &Type {
    loop {
        match ty {
            Type::Borrow(inner) => ty = inner,
            Type::Pointer(inner) => ty = inner,
            _ => return ty,
        }
    }
}

/// Zig `ordinaryIndexAddressTargetType` (lowering_rules.zig:148).
fn ordinary_index_address_target_type(ty: &Type) -> Option<&Type> {
    let target = peel_borrow_pointer_type(ty);
    match target {
        Type::Array(_) => Some(target),
        Type::UserDefined(ud) if ud.name == "Slice" && ud.generics.len() == 1 => Some(target),
        _ => None,
    }
}

/// Zig `planAddressOf` (lowering_rules.zig:5870), shape only — the
/// piece `importedMacroArgAddressShape` (sab_codegen.zig:14466)
/// consumes after resolving the deref-source / index-target types.
pub fn plan_address_of_shape(
    expr: &Node,
    deref_source_ty: Option<&Type>,
    index_target_ty: Option<&Type>,
) -> AddressOfShape {
    match expr {
        Node::Identifier(_) => AddressOfShape::Identifier,
        Node::DerefExpr(_) => {
            let source_ty = match deref_source_ty {
                Some(ty) => ty,
                None => return AddressOfShape::ValueTemp,
            };
            if plain_borrow_or_pointer_address_source(source_ty) {
                return AddressOfShape::DerefBorrowOrPointer;
            }
            if smart_pointer_deref_present(source_ty) {
                return AddressOfShape::DerefSmartPointer;
            }
            AddressOfShape::ValueTemp
        }
        Node::FieldExpr(_) => AddressOfShape::Field,
        Node::IndexExpr(_) => match index_target_ty {
            Some(target_ty) if ordinary_index_address_target_type(target_ty).is_some() => {
                AddressOfShape::Index
            }
            _ => AddressOfShape::ValueTemp,
        },
        _ => AddressOfShape::ValueTemp,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sla_ast::{ArrayType, CallExpr, TupleType, UserDefined};

    fn plan(borrowed: u64, addr_slot: u64, expr_out: bool, leading: usize) -> ImportedMacroCallPlan {
        ImportedMacroCallPlan {
            macro_name: "M".to_string(),
            import_path: Some("sa_std/m.sa".to_string()),
            arity: 4,
            leading_outputs: leading,
            borrowed_arg_mask: borrowed,
            address_slot_arg_mask: addr_slot,
            expression_output: expr_out,
        }
    }

    fn ident(name: &str) -> Node {
        Node::Identifier(name.to_string())
    }

    fn i32_ty() -> Type {
        Type::Primitive(Primitive::I32)
    }

    fn ptr_ty() -> Type {
        Type::Pointer(Box::new(i32_ty()))
    }

    #[test]
    fn param_index_shifts_only_for_expression_output() {
        let p = plan(0, 0, true, 2);
        assert_eq!(p.macro_param_index_for_call_arg(0), 2);
        assert_eq!(p.macro_param_index_for_call_arg(3), 5);
        let q = plan(0, 0, false, 2);
        assert_eq!(q.macro_param_index_for_call_arg(3), 3);
    }

    #[test]
    fn addressable_slot_masks_and_64_cap() {
        let p = plan(0b0100, 0b0010, false, 0);
        assert!(!p.call_arg_needs_addressable_slot(0));
        assert!(p.call_arg_needs_addressable_slot(1));
        assert!(p.call_arg_needs_addressable_slot(2));
        assert!(p.call_arg_needs_direct_address_slot(1));
        assert!(!p.call_arg_needs_direct_address_slot(2));
        // Index at/after 64 never needs a slot (Zig guard).
        let wide = plan(u64::MAX, u64::MAX, true, 1);
        assert!(!wide.call_arg_needs_addressable_slot(63));
        assert!(!wide.call_arg_needs_direct_address_slot(63));
    }

    #[test]
    fn addressable_identifier_name_requires_slot_and_identifier() {
        let p = plan(0b1, 0, false, 0);
        assert_eq!(p.addressable_identifier_arg_name(0, &ident("x")), Some("x"));
        assert_eq!(p.addressable_identifier_arg_name(1, &ident("x")), None);
    }

    #[test]
    fn direct_slot_action_matrix() {
        use ImportedMacroAddressableArgAction as A;
        let p = plan(0, 0b1, false, 0);
        assert_eq!(
            p.plan_address_expression_arg_action(0, AddressOfShape::Identifier, true),
            A::ReuseExistingAddressable
        );
        assert_eq!(
            p.plan_address_expression_arg_action(0, AddressOfShape::Identifier, false),
            A::MaterializeStackSlot
        );
        assert_eq!(
            p.plan_address_expression_arg_action(0, AddressOfShape::Field, false),
            A::PassAddressExpression
        );
        assert_eq!(
            p.plan_address_expression_arg_action(0, AddressOfShape::ValueTemp, false),
            A::PassValue
        );
        // No slot needed at all.
        assert_eq!(
            p.plan_address_expression_arg_action(1, AddressOfShape::Field, false),
            A::PassValue
        );
    }

    #[test]
    fn borrowed_slot_action_matrix() {
        use ImportedMacroAddressableArgAction as A;
        let p = plan(0b1, 0, false, 0);
        assert_eq!(
            p.plan_address_expression_arg_action(0, AddressOfShape::Field, false),
            A::MaterializeAddressExpressionStackSlot
        );
        assert_eq!(
            p.plan_address_expression_arg_action(0, AddressOfShape::ValueTemp, false),
            A::MaterializeStackSlot
        );
        assert_eq!(p.plan_addressable_arg_action(0, true), A::ReuseExistingAddressable);
    }

    #[test]
    fn lowering_action_pointer_backed_projection_and_deref() {
        use ImportedMacroArgLoweringAction as A;
        let p = plan(0b1, 0, false, 0);
        // Pointer-ABI field/index projections pass by value.
        assert_eq!(
            p.plan_addressable_arg_lowering_action(0, AddressOfShape::Field, false, &ptr_ty()),
            A::PassPointerBackedProjection
        );
        assert_eq!(
            p.plan_addressable_arg_lowering_action(0, AddressOfShape::Index, false, &ptr_ty()),
            A::PassPointerBackedProjection
        );
        // Pointer-ABI derefs pass the address expression itself.
        assert_eq!(
            p.plan_addressable_arg_lowering_action(
                0,
                AddressOfShape::DerefBorrowOrPointer,
                false,
                &ptr_ty()
            ),
            A::PassAddressExpression
        );
        // Scalar field under a borrowed slot materializes instead.
        assert_eq!(
            p.plan_addressable_arg_lowering_action(0, AddressOfShape::Field, false, &i32_ty()),
            A::MaterializeAddressExpressionStackSlot
        );
    }

    #[test]
    fn bypass_action_matrix() {
        use ImportedMacroArgLoweringAction as A;
        // No slot: always pass value.
        let p = plan(0, 0, false, 0);
        assert_eq!(p.plan_arg_value_bypass_action(0, &ident("x"), &i32_ty()), Some(A::PassValue));
        // Borrowed slot + raw-pointer type: raw pointer bypass.
        let p = plan(0b1, 0, false, 0);
        assert_eq!(
            p.plan_arg_value_bypass_action(0, &ident("x"), &Type::Primitive(Primitive::RawPtr)),
            Some(A::PassRawPointerValue)
        );
        // Borrowed slot + pointer-typed identifier: the raw-pointer
        // check precedes the abi-pointer check in Zig, so this is a
        // raw-pointer bypass, not a plain pass-value.
        assert_eq!(
            p.plan_arg_value_bypass_action(0, &ident("x"), &ptr_ty()),
            Some(A::PassRawPointerValue)
        );
        // Borrowed slot + abi-pointer (user-defined) identifier:
        // pass value.
        let ud_ty = Type::UserDefined(UserDefined {
            name: "S".to_string(),
            generics: vec![],
        });
        assert_eq!(p.plan_arg_value_bypass_action(0, &ident("x"), &ud_ty), Some(A::PassValue));
        // Borrowed slot + scalar identifier: no bypass.
        assert_eq!(p.plan_arg_value_bypass_action(0, &ident("x"), &i32_ty()), None);
        // Direct slot + infer-typed identifier: pass value.
        let p = plan(0, 0b1, false, 0);
        assert_eq!(
            p.plan_arg_value_bypass_action(0, &ident("x"), &Type::Infer),
            Some(A::PassValue)
        );
    }

    #[test]
    fn raw_pointer_value_detection() {
        assert!(imported_macro_borrow_uses_raw_pointer_value(&ptr_ty()));
        assert!(imported_macro_borrow_uses_raw_pointer_value(&Type::Borrow(Box::new(i32_ty()))));
        assert!(!imported_macro_borrow_uses_raw_pointer_value(&i32_ty()));
        let call = Node::CallExpr(CallExpr {
            func_name: "FOO_AS_PTR".to_string(),
            associated_target: None,
            generics: vec![],
            args: vec![],
            callee: None,
        });
        assert!(imported_macro_arg_uses_raw_pointer_value(&call, &i32_ty()));
        assert!(!imported_macro_arg_uses_raw_pointer_value(&ident("x"), &i32_ty()));
    }

    #[test]
    fn abi_pointer_set() {
        assert!(abi_passes_as_pointer(&ptr_ty()));
        assert!(abi_passes_as_pointer(&Type::Tuple(TupleType { elems: vec![] })));
        assert!(abi_passes_as_pointer(&Type::Array(ArrayType {
            elem: Box::new(i32_ty()),
            len: 2
        })));
        assert!(abi_passes_as_pointer(&Type::UserDefined(UserDefined {
            name: "S".to_string(),
            generics: vec![]
        })));
        assert!(!abi_passes_as_pointer(&i32_ty()));
        assert!(!abi_passes_as_pointer(&Type::Infer));
    }

    #[test]
    fn consumes_value_arg_names() {
        assert!(imported_macro_direct_call_consumes_value_arg("FS_READ_BUFFER_FREE", 0));
        assert!(imported_macro_direct_call_consumes_value_arg("SLA_FS_BUFFER_FREE", 0));
        assert!(!imported_macro_direct_call_consumes_value_arg("FS_READ_BUFFER_FREE", 1));
        assert!(!imported_macro_direct_call_consumes_value_arg("OTHER", 0));
    }

    #[test]
    fn address_of_shape_matrix() {
        use sla_ast::{DerefExpr, FieldExpr, IndexExpr};
        assert_eq!(plan_address_of_shape(&ident("x"), None, None), AddressOfShape::Identifier);
        let deref = Node::DerefExpr(DerefExpr { expr: Box::new(ident("p")) });
        assert_eq!(
            plan_address_of_shape(&deref, Some(&ptr_ty()), None),
            AddressOfShape::DerefBorrowOrPointer
        );
        let box_ty = Type::UserDefined(UserDefined {
            name: "Box".to_string(),
            generics: vec![i32_ty()],
        });
        assert_eq!(
            plan_address_of_shape(&deref, Some(&box_ty), None),
            AddressOfShape::DerefSmartPointer
        );
        assert_eq!(plan_address_of_shape(&deref, None, None), AddressOfShape::ValueTemp);
        let field = Node::FieldExpr(FieldExpr {
            expr: Box::new(ident("s")),
            field_name: "f".to_string(),
        });
        assert_eq!(plan_address_of_shape(&field, None, None), AddressOfShape::Field);
        let index = Node::IndexExpr(IndexExpr {
            target: Box::new(ident("a")),
            index: Box::new(ident("i")),
        });
        let arr_ty = Type::Array(ArrayType {
            elem: Box::new(i32_ty()),
            len: 4,
        });
        assert_eq!(
            plan_address_of_shape(&index, None, Some(&arr_ty)),
            AddressOfShape::Index
        );
        assert_eq!(
            plan_address_of_shape(&index, None, Some(&i32_ty())),
            AddressOfShape::ValueTemp
        );
    }
}
