//! The `Codegen` state and its small shared emitters, mirrored from
//! the state block and helper methods of codegen.zig's `Codegen`
//! struct (codegen.zig:96, 331-398, 620-703, 786-841, 9724-9765,
//! 10437-10460).
//!
//! State that the Zig original threads as parameters or keeps in
//! handle-family tables the slice never populates (RefCell borrow
//! handles, mutex/file/metadata handle maps, future/task tables) is
//! not carried here; the release cascade documents where those checks
//! sat. `hoisted_allocs` is per-function state instead of a threaded
//! parameter (restructuring note, divergences (b)).

use std::collections::{HashMap, HashSet};

use sla_ast::{FuncDecl, Literal, Node, Primitive, Type};
use sla_lowering::{
    abi_return_type_string, abi_type_size, abi_type_string, expr_result_needs_release,
    is_main_name, CheckInfo,
};

use crate::{unsupported, Result, TextError};

pub struct Codegen<'c> {
    pub(crate) check: &'c dyn CheckInfo,
    pub(crate) out: String,

    // Global counters: never reset between functions (goldens show
    // tmp/label/string numbering continuing across the whole file).
    pub(crate) tmp_idx: usize,
    pub(crate) label_idx: usize,
    pub(crate) string_idx: usize,
    #[allow(dead_code)]
    pub(crate) macro_local_idx: usize,

    // Per-function binding state (cleared at each func/test decl).
    pub(crate) consumed_bindings: HashSet<String>,
    pub(crate) stack_alloc_bindings: HashSet<String>,
    pub(crate) assigned_value_slots: HashSet<String>,
    pub(crate) addressable_bindings: HashSet<String>,
    pub(crate) assigned_bindings: HashSet<String>,
    pub(crate) repeated_let_bindings: HashSet<String>,
    pub(crate) binding_aliases: Vec<(String, Vec<String>)>,
    /// codegen.zig `borrow_source_temps`: address temp -> the
    /// source temp its value derives from, recorded by the
    /// place-address consumers (remember*Source) and consumed by
    /// emitRelease's borrow-address-temp cascade step.
    pub(crate) borrow_source_temps: HashMap<String, String>,
    /// codegen.zig `hashmap_key_slots`: string literal -> the
    /// cached slice-slot register materialised for it in this
    /// function (identity-keyed map/set probes for the same
    /// literal must hit the same slot).
    pub(crate) hashmap_key_slots: HashMap<String, String>,
    pub(crate) let_binding_aliases: HashMap<usize, String>,
    pub(crate) local_binding_types: Vec<(String, Type)>,
    pub(crate) hoisted_allocs: Vec<String>,

    // Program-wide state.
    pub(crate) global_const_bindings: HashSet<String>,
    pub(crate) global_scalar_consts: Vec<(String, Literal)>,
    pub(crate) func_index: Vec<(String, FuncDecl)>,
    /// Impl / overload methods by mangled name (`Type_method`,
    /// `Type__Trait_method`), collected in `generate_program`
    /// alongside the emission of their bodies. Kept separate from
    /// `func_index` so bare-name and tail-match resolution keep
    /// their existing ambiguity behavior; associated calls and
    /// receiver-typed method calls consult this index.
    pub(crate) assoc_index: Vec<(String, FuncDecl)>,
    /// Enum declarations by name, collected from the program's
    /// decls in `generate_program` (the SAB driver keeps the same
    /// index; the checker's own enum table keeps only variant
    /// names, not fields/discriminants).
    pub(crate) enum_index: Vec<(String, sla_ast::EnumDecl)>,

    // Loop state.
    pub(crate) loop_break_labels: Vec<String>,
    pub(crate) loop_continue_labels: Vec<String>,
    pub(crate) loop_body_local_scopes: Vec<Vec<String>>,
    pub(crate) loop_body_block_depths: Vec<usize>,
}

impl<'c> Codegen<'c> {
    pub fn new(check: &'c dyn CheckInfo) -> Self {
        Codegen {
            check,
            out: String::new(),
            tmp_idx: 0,
            label_idx: 0,
            string_idx: 0,
            macro_local_idx: 0,
            consumed_bindings: HashSet::new(),
            stack_alloc_bindings: HashSet::new(),
            assigned_value_slots: HashSet::new(),
            addressable_bindings: HashSet::new(),
            assigned_bindings: HashSet::new(),
            repeated_let_bindings: HashSet::new(),
            binding_aliases: Vec::new(),
            borrow_source_temps: HashMap::new(),
            hashmap_key_slots: HashMap::new(),
            let_binding_aliases: HashMap::new(),
            local_binding_types: Vec::new(),
            hoisted_allocs: Vec::new(),
            global_const_bindings: HashSet::new(),
            global_scalar_consts: Vec::new(),
            func_index: Vec::new(),
            assoc_index: Vec::new(),
            enum_index: Vec::new(),
            loop_break_labels: Vec::new(),
            loop_continue_labels: Vec::new(),
            loop_body_local_scopes: Vec::new(),
            loop_body_block_depths: Vec::new(),
        }
    }

    // -- output ------------------------------------------------------

    /// Raw append (Zig `outPrint` with the format's own newlines).
    pub(crate) fn w(&mut self, s: &str) {
        self.out.push_str(s);
    }

    /// Append one line (content + `\n`).
    pub(crate) fn wl(&mut self, s: &str) {
        self.out.push_str(s);
        self.out.push('\n');
    }

    /// Append an indented instruction line (4 spaces, as every Zig
    /// instruction format string carries).
    pub(crate) fn ins(&mut self, s: &str) {
        self.out.push_str("    ");
        self.out.push_str(s);
        self.out.push('\n');
    }

    // -- counters (codegen.zig:367-398) ------------------------------

    pub(crate) fn new_tmp(&mut self) -> String {
        let name = format!("tmp_{}", self.tmp_idx);
        self.tmp_idx += 1;
        name
    }

    pub(crate) fn new_label(&mut self, prefix: &str) -> String {
        let name = format!("{prefix}_{}", self.label_idx);
        self.label_idx += 1;
        name
    }

    pub(crate) fn new_string_const(&mut self) -> String {
        let name = format!("SLA_STR_{}", self.string_idx);
        self.string_idx += 1;
        name
    }

    // -- binding aliases (codegen.zig:331-359) ------------------------

    pub(crate) fn resolve_binding_name(&self, name: &str) -> String {
        for (n, aliases) in &self.binding_aliases {
            if n == name {
                if let Some(top) = aliases.last() {
                    return top.clone();
                }
            }
        }
        name.to_string()
    }

    pub(crate) fn push_binding_alias(&mut self, name: &str) -> String {
        let alias = self.new_tmp();
        self.push_binding_alias_to(name, &alias);
        alias
    }

    pub(crate) fn push_binding_alias_to(&mut self, name: &str, alias: &str) {
        for (n, aliases) in &mut self.binding_aliases {
            if n == name {
                aliases.push(alias.to_string());
                return;
            }
        }
        self.binding_aliases
            .push((name.to_string(), vec![alias.to_string()]));
    }

    pub(crate) fn pop_binding_alias(&mut self, name: &str) {
        for (n, aliases) in &mut self.binding_aliases {
            if n == name {
                aliases.pop();
                return;
            }
        }
    }

    pub(crate) fn clear_binding_aliases(&mut self) {
        self.binding_aliases.clear();
    }

    // -- per-function state reset (genFuncDeclNamed/genTestDecl) ------

    pub(crate) fn reset_function_state(&mut self) {
        self.addressable_bindings.clear();
        self.assigned_bindings.clear();
        self.assigned_value_slots.clear();
        self.repeated_let_bindings.clear();
        self.stack_alloc_bindings.clear();
        self.consumed_bindings.clear();
        self.borrow_source_temps.clear();
        self.hashmap_key_slots.clear();
        self.local_binding_types.clear();
        self.clear_binding_aliases();
        self.let_binding_aliases.clear();
        self.hoisted_allocs.clear();
    }

    // -- lookups --------------------------------------------------------

    /// `tc.funcs` first (checker-registered, incl. imported), then the
    /// program's own declarations.
    pub(crate) fn func_decl(&self, name: &str) -> Option<FuncDecl> {
        if let Some(f) = self.check.func(name) {
            return Some(f.clone());
        }
        for (n, f) in &self.func_index {
            if n == name {
                return Some(f.clone());
            }
        }
        None
    }

    /// `using` extension callee lookup (mirrors the SAB driver's
    /// plain-call tail): the call site keeps the bare method name
    /// while the import expansion lifted the callee under a
    /// module-prefixed key (`math3__double`); a UNIQUE tail-segment
    /// match over the program's function index is the callee.
    /// Ambiguity or absence returns None and the caller keeps its
    /// existing fallback.
    pub(crate) fn func_decl_tail_match(&self, name: &str) -> Option<(String, FuncDecl)> {
        let mut found: Option<&(String, FuncDecl)> = None;
        for entry in &self.func_index {
            let key = &entry.0;
            let tail_hit = key.rsplit("::").next() == Some(name)
                || key.rsplit("__").next() == Some(name);
            if tail_hit {
                if found.is_some() {
                    return None;
                }
                found = Some(entry);
            }
        }
        found.map(|(n, f)| (n.clone(), f.clone()))
    }

    /// Enum declaration from the program index (collected in
    /// `generate_program`, including imported modules' decls).
    pub(crate) fn enum_decl(&self, name: &str) -> Option<&sla_ast::EnumDecl> {
        self.enum_index
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, d)| d)
    }

    pub(crate) fn expr_type(&self, node: &Node) -> Option<Type> {
        self.check.expr_type(node).cloned()
    }

    pub(crate) fn require_expr_type(&self, node: &Node, context: &'static str) -> Result<Type> {
        self.expr_type(node).ok_or(TextError::MissingType {
            context: context.to_string(),
        })
    }

    pub(crate) fn type_is_copy_value(&self, ty: &Type) -> bool {
        self.check.type_is_copy_value(ty)
    }

    pub(crate) fn type_is_copy_struct(&self, ty: &Type) -> bool {
        self.check.type_is_copy_struct(ty)
    }

    pub(crate) fn remember_local_binding_type(&mut self, name: &str, ty: &Type) {
        for (n, t) in &mut self.local_binding_types {
            if n == name {
                *t = ty.clone();
                return;
            }
        }
        self.local_binding_types.push((name.to_string(), ty.clone()));
    }

    pub(crate) fn local_binding_type_for_name(&self, name: &str) -> Option<Type> {
        self.local_binding_types
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, t)| t.clone())
    }

    pub(crate) fn global_scalar_const(&self, name: &str) -> Option<Literal> {
        self.global_scalar_consts
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, lit)| lit.clone())
    }

    pub(crate) fn mark_consumed_binding(&mut self, name: &str) {
        self.consumed_bindings.insert(name.to_string());
    }

    // -- symbol / type spellings ---------------------------------------

    /// codegen.zig:628 `loweredFuncSymbol`.
    pub(crate) fn lowered_func_symbol(&self, name: &str) -> String {
        if is_main_name(name) {
            return name.to_string();
        }
        if let Some(func) = self.func_decl(name) {
            if func.is_extern || func.no_mangle {
                return name.to_string();
            }
        }
        if self.check.is_extern_func(name) || name.starts_with("sa_") {
            return name.to_string();
        }
        format!("sla__{name}")
    }

    pub(crate) fn fn_ptr_vtable_name(&self, func_name: &str) -> String {
        format!("SLA_FNPTR_VT_{func_name}")
    }

    /// codegen.zig:671 `emitFunctionPointerVTableDecl`.
    pub(crate) fn emit_function_pointer_vtable_decl(&mut self, func_name: &str) {
        let vt_name = self.fn_ptr_vtable_name(func_name);
        let lowered = self.lowered_func_symbol(func_name);
        self.wl(&format!("@const {vt_name} = vtable {{ call = @{lowered} }}"));
    }
}

/// codegen.zig:2455 `typeSize` (thins to lowering `abiTypeSize`).
pub(crate) fn type_size(ty: &Type) -> usize {
    abi_type_size(ty)
}

/// codegen.zig:2467 `typeString` (thins to lowering `abiTypeString`).
pub(crate) fn type_string(ty: &Type) -> &'static str {
    abi_type_string(ty)
}

/// codegen.zig:2491 `abiReturnTypeString` re-export path.
pub(crate) fn ret_type_string(ty: &Type) -> &'static str {
    abi_return_type_string(ty)
}

impl<'c> Codegen<'c> {
    // -- const emitters (codegen.zig:792-841) --------------------------

    /// codegen.zig:792 `emitIntConst`.
    pub(crate) fn emit_int_const(&mut self, target: &str, value: i64) {
        self.ins(&format!("{target} = {value}"));
    }

    /// codegen.zig:796 `emitFloatConst`. Zig `{d}` on f64 prints the
    /// decimal form; integral finite values get an explicit `.0`.
    /// Rust's Display for f64 is shortest-round-trip, which can differ
    /// from Zig's decimal expansion for non-integral values — no float
    /// literal appears in the slice; flagged in the divergences log.
    pub(crate) fn emit_float_const(&mut self, target: &str, value: f64) {
        if value.is_finite() && value.floor() == value {
            self.ins(&format!("{target} = {}.0", value as i64));
        } else {
            self.ins(&format!("{target} = {value}"));
        }
    }

    /// codegen.zig:804 `genLiteralValue`.
    pub(crate) fn gen_literal_value(&mut self, lit: &Literal) -> Result<String> {
        let reg = self.new_tmp();
        match lit {
            Literal::IntVal(v) => self.emit_int_const(&reg, *v),
            Literal::FloatVal(v) => self.emit_float_const(&reg, *v),
            Literal::BoolVal(v) => self.emit_int_const(&reg, if *v { 1 } else { 0 }),
            Literal::StringVal(v) => {
                let label = self.new_string_const();
                // One trailing NUL so the constant can double as a C
                // string; escapedStringByteLen excludes it.
                self.ins(&format!("@const {label} = utf8:\"{v}\\0\""));
                let len_reg = self.new_tmp();
                self.emit_int_const(&len_reg, escaped_string_byte_len(v) as i64);
                self.stack_alloc_bindings.insert(reg.clone());
                self.ins(&format!("{reg} = stack_alloc Slice_SIZE"));
                self.ins(&format!("EXPAND SLICE_NEW {reg}, &{label}, {len_reg}"));
                self.emit_release(&len_reg)?;
            }
        }
        Ok(reg)
    }

    // -- copies and conditions -----------------------------------------

    /// codegen.zig:9724 `emitPrimitiveCopy`.
    pub(crate) fn emit_primitive_copy(
        &mut self,
        target: &str,
        source: &str,
        ty: &Type,
    ) -> Result<()> {
        match ty {
            Type::Primitive(sla_ast::Primitive::Boolean) => {
                self.ins(&format!("{target} = or {source}, 0"));
            }
            Type::Primitive(
                sla_ast::Primitive::F32 | sla_ast::Primitive::F64 | sla_ast::Primitive::Float,
            ) => {
                // fadd against a materialized float zero (an integer
                // `add` copy would be folded onto the source register).
                let zero = self.new_tmp();
                self.emit_float_const(&zero, 0.0);
                self.ins(&format!("{target} = fadd {source}, {zero}"));
                self.emit_release(&zero)?;
            }
            Type::Primitive(_) => {
                self.ins(&format!("{target} = add {source}, 0"));
            }
            _ => return Err(TextError::MissingType { context: "emitPrimitiveCopy non-primitive".to_string() }),
        }
        Ok(())
    }

    /// codegen.zig:9744 `genBranchConditionReg`: a bare primitive
    /// identifier condition is copied into a fresh temp so both branch
    /// successors can release it; anything else is used as generated.
    pub(crate) fn gen_branch_condition_reg(&mut self, cond: &Node) -> Result<String> {
        let cond_reg = self.gen_expr(cond)?;
        let cond_ty = self.require_expr_type(cond, "branch condition")?;
        if let Node::Identifier(name) = cond {
            if matches!(cond_ty, Type::Primitive(_)) {
                let resolved = self.resolve_binding_name(name);
                if cond_reg == resolved
                    || cond_reg == *name
                    || self.global_const_bindings.contains(name)
                {
                    let copied = self.new_tmp();
                    self.emit_primitive_copy(&copied, &cond_reg, &cond_ty)?;
                    return Ok(copied);
                }
            }
        }
        Ok(cond_reg)
    }

    /// codegen.zig:10441 `exprResultRegNeedsRelease`: the plain
    /// lowering fact plus the cast materialization rule.
    pub(crate) fn expr_result_reg_needs_release(&self, expr: &Node) -> bool {
        if let Node::CastExpr(cast) = expr {
            let src_ty = self.expr_type(&cast.expr);
            return match src_ty {
                Some(src) => {
                    sla_lowering::cast_result_materializes_temp(&src, &cast.ty)
                }
                None => expr_result_needs_release(expr),
            };
        }
        expr_result_needs_release(expr)
    }

    /// Deep-copy emission for Copy structs (codegen.zig:3913
    /// `genCopyValueInto`), plain-field subset in copy.rs: a
    /// record whose fields are all plain words or nested plain
    /// structs copies field by field into a fresh record. A
    /// heap-handle field needs the deep clone dispatcher and
    /// stays loud.
    pub(crate) fn gen_copy_value_into(
        &mut self,
        target: &str,
        source: &str,
        ty: &Type,
    ) -> Result<()> {
        if self.gen_copy_value_into_inner(target, source, ty)? {
            Ok(())
        } else {
            unsupported(
                "genCopyValueInto (Copy-struct deep copy of handle fields)",
                "codegen.zig:3913",
            )
        }
    }
}

/// codegen.zig:2420 `sanitizeTestName`.
pub(crate) fn sanitize_test_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        match c {
            '(' | ')' => out.push('-'),
            '"' | '\\' | '\n' | '\r' | '\t' => out.push(' '),
            _ => out.push(c),
        }
    }
    out
}

/// codegen.zig:3063 `escapedStringByteLen`: byte length of the value
/// as SA will store it — a backslash escape pair counts once. The AST
/// string value retains source escape text (two bytes), which is why
/// this differs from `value.len()`.
pub(crate) fn escaped_string_byte_len(value: &str) -> usize {
    let bytes = value.as_bytes();
    let mut len = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 1;
        }
        len += 1;
        i += 1;
    }
    len
}

/// codegen.zig:3055/3059 shape facts used by the println and
/// format emitters, mirroring lowering_rules.isFormatStringType /
/// isStringLikeType exactly (borrow/pointer peeled first).
pub(crate) fn is_format_string_type(ty: &Type) -> bool {
    // lowering_rules.isFormatStringType: peel borrow/pointer,
    // then the user-defined `String`.
    matches!(
        sla_lowering::peel_borrow_pointer_type(ty),
        Type::UserDefined(ud) if ud.name == "String"
    )
}

pub(crate) fn is_string_like_type(ty: &Type) -> bool {
    // lowering_rules.isStringLikeType (5280): peel borrow/pointer,
    // then raw pointers, arrays and `String` are string-like. The
    // earlier reduced form (Borrow(Array) / `str` only) missed
    // raw-pointer-typed string literals in format/println args.
    match sla_lowering::peel_borrow_pointer_type(ty) {
        Type::Primitive(Primitive::RawPtr) => true,
        Type::Array(_) => true,
        Type::UserDefined(ud) => ud.name == "String",
        _ => false,
    }
}
