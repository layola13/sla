//! Overload unification (R-6 follow-up), mirroring the Zig
//! checker's cross-source candidate discipline (staging
//! type_checker.zig:3220-3560) and the extern contract member
//! identity rules (:5777-5825).
//!
//! Registration (check_program.rs pass 2 and the contract front
//! ends) promotes same-name declarations into candidate sets:
//!   * `.sai` contracts via `Checker::register_extern_contract`;
//!   * source-level `extern "C"` prototypes and their `@no_mangle`
//!     definitions pair per signature class via
//!     [`Checker::register_source_extern`];
//!   * imported signatures via
//!     [`Checker::register_imported_overload_member`].
//! A call then resolves against ONE candidate pool built from the
//! func / imported / extern sources ([`Checker::try_resolve_unified_call`]):
//! argument types are probed under a scope-state snapshot and
//! rolled back (probing never consumes), each candidate matches by
//! its own rule — func/imported members by `overloadParamMatches`,
//! extern members by the spelling-based `contractParamMatches` —
//! and exactly one winner is dispatched to its real argument check.
//! Zero winners are a TypeMismatch naming the argument types and
//! the candidate list; several winners are an AmbiguousCall.

use crate::checker::{
    extern_ret_type, Checker, ExternParam, ExternSig, FuncSig,
};
use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use crate::types_util::{is_integer_type, type_desc};
use sla_ast::decls::FuncDecl;
use sla_ast::exprs::{CallExpr, Literal};
use sla_ast::node::Node;
use sla_ast::types::{Primitive, Type};

/// One resolution candidate (Zig `OverloadCandidate`). Func and
/// imported members carry the same FuncSig shape; extern members
/// carry the contract signature.
enum Candidate {
    Func(FuncSig),
    Imported(FuncSig),
    Extern(ExternSig),
}

impl Candidate {
    fn fixed_arity(&self) -> usize {
        match self {
            Candidate::Func(sig) | Candidate::Imported(sig) => sig.params.len(),
            Candidate::Extern(sig) => sig.params.len(),
        }
    }

    fn render(&self, name: &str) -> String {
        match self {
            Candidate::Func(sig) | Candidate::Imported(sig) => {
                crate::check_overload::render_sig(name, sig)
            }
            Candidate::Extern(sig) => {
                let params = sig
                    .params
                    .iter()
                    .map(|p| p.ty.trim())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "{name}({params}) -> {}",
                    sig.ret_abi.trim()
                )
            }
        }
    }
}

/// Zig `appendOverloadTypeTag`: the member-key tag of one TYPE.
fn append_overload_type_tag(tag: &mut String, ty: &Type) {
    match ty {
        Type::Primitive(p) => tag.push_str(match p {
            Primitive::I8 => "i8",
            Primitive::I16 => "i16",
            Primitive::I32 => "i32",
            Primitive::I64 => "i64",
            Primitive::U8 => "u8",
            Primitive::U16 => "u16",
            Primitive::U32 => "u32",
            Primitive::U64 => "u64",
            Primitive::F32 => "f32",
            Primitive::F64 => "f64",
            Primitive::Boolean => "bool",
            Primitive::Integer => "int",
            Primitive::Float => "f64",
            Primitive::VoidType => "void",
            Primitive::RawPtr => "ptr",
            Primitive::VaList => "va_list",
            Primitive::Isize => "i64",
            Primitive::Usize => "u64",
        }),
        Type::Pointer(_) => tag.push('*'),
        Type::Borrow(inner) => {
            tag.push('&');
            append_overload_type_tag(tag, inner);
        }
        Type::UserDefined(u) => tag.push_str(&u.name),
        Type::Array(a) => {
            append_overload_type_tag(tag, &a.elem);
            tag.push_str(&format!("[{}]", a.len));
        }
        Type::Tuple(t) => {
            tag.push_str("tuple");
            for e in &t.elems {
                append_overload_type_tag(tag, e);
            }
        }
        Type::FnPtr(_) => tag.push_str("fnptr"),
        Type::Closure(_) => tag.push_str("closure"),
        _ => tag.push_str("unknown"),
    }
}

/// Zig `overloadMemberKeyForParams` (staging :2730): the member
/// key of a SOURCE-level overload member from its parameter types
/// and capability flags — `<name>__ov_<tag>`, tags joined by '_',
/// 'b'/'m' appended for borrow/move parameters.
pub fn overload_member_key_for_params(
    name: &str,
    params: &[crate::checker::ParamSig],
) -> String {
    let mut tag = String::new();
    if params.is_empty() {
        tag.push('v');
    } else {
        for (i, param) in params.iter().enumerate() {
            if i > 0 {
                tag.push('_');
            }
            append_overload_type_tag(&mut tag, &param.ty);
            if param.is_borrow {
                tag.push('b');
            }
            if param.is_move {
                tag.push('m');
            }
        }
    }
    format!("{name}__ov_{tag}")
}

/// Zig `externMemberKey` (staging :5788): the member key of a
/// contract member from its parameter SPELLINGS: every
/// non-alphanumeric character becomes '_', with 'b'/'m'/'r'
/// appended for borrow/move/raw parameters; zero parameters map
/// to 'v'; a variadic declaration appends `_va_list`.
pub fn extern_member_key(name: &str, sig: &ExternSig) -> String {
    let mut tag = String::new();
    if sig.params.is_empty() {
        tag.push('v');
    } else {
        for (i, param) in sig.params.iter().enumerate() {
            if i > 0 {
                tag.push('_');
            }
            for ch in param.ty.trim().chars() {
                tag.push(if ch.is_alphanumeric() { ch } else { '_' });
            }
            if param.is_borrow {
                tag.push('b');
            }
            if param.is_move {
                tag.push('m');
            }
            if param.is_raw {
                tag.push('r');
            }
        }
    }
    if sig.is_variadic {
        tag.push_str("_va_list");
    }
    format!("{name}__ov_{tag}")
}

/// Convert an extern contract parameter spelling to the AST type the
/// member-symbol computation needs (Root-3 follow-up: an extern
/// overload member must record the winning member's symbol exactly
/// as `decl_symbol` (codegen-text) recomputes it from the
/// definition's parameter types). The mapping is the IDENTITY of
/// `sla_ast::overload_type_tag`'s arms, so the two sides agree by
/// construction: `isize`/`usize` stay Isize/Usize (NOT the I64/U64
/// collapse of `primitive_from_spelling`), `ptr`/`raw_ptr` map to
/// RawPtr, `*T` to Pointer, `&T` to Borrow, other bare identifiers
/// to UserDefined. Returns None for a spelling with no AST
/// counterpart — the caller then skips recording (today's bare-name
/// behaviour) rather than recording a symbol the emitter cannot
/// match.
fn extern_spelling_to_type(spelling_raw: &str) -> Option<Type> {
    let spelling = spelling_raw.trim();
    if let Some(inner) = spelling
        .strip_prefix('*')
        .or_else(|| spelling.strip_prefix('^'))
    {
        let inner_ty = extern_spelling_to_type(inner)?;
        return Some(Type::Pointer(Box::new(inner_ty)));
    }
    if let Some(inner) = spelling.strip_prefix('&') {
        let inner_ty = extern_spelling_to_type(inner)?;
        return Some(Type::Borrow(Box::new(inner_ty)));
    }
    Some(match spelling {
        "i8" => Type::Primitive(Primitive::I8),
        "i16" => Type::Primitive(Primitive::I16),
        "i32" => Type::Primitive(Primitive::I32),
        "i64" => Type::Primitive(Primitive::I64),
        "isize" => Type::Primitive(Primitive::Isize),
        "u8" => Type::Primitive(Primitive::U8),
        "u16" => Type::Primitive(Primitive::U16),
        "u32" => Type::Primitive(Primitive::U32),
        "u64" => Type::Primitive(Primitive::U64),
        "usize" => Type::Primitive(Primitive::Usize),
        "f32" => Type::Primitive(Primitive::F32),
        "f64" => Type::Primitive(Primitive::F64),
        "integer" => Type::Primitive(Primitive::Integer),
        "float" => Type::Primitive(Primitive::Float),
        "bool" | "boolean" => Type::Primitive(Primitive::Boolean),
        "ptr" | "raw_ptr" => Type::Primitive(Primitive::RawPtr),
        "void" | "void_type" => Type::Primitive(Primitive::VoidType),
        other => {
            if !other.is_empty()
                && other.chars().all(|c| c.is_alphanumeric() || c == '_')
            {
                Type::UserDefined(sla_ast::types::UserDefined {
                    name: other.to_string(),
                    generics: Vec::new(),
                })
            } else {
                return None;
            }
        }
    })
}

/// The mangled member symbol of one extern overload-set member
/// (Root-3 follow-up): the member's fixed parameter types, with a
/// trailing VaList marker iff the contract is variadic. The marker
/// is REQUIRED: a fixed/variadic pair with identical fixed
/// parameters (e.g. `pick(x: ptr)` vs `pick(x: ptr, ...)`) would
/// otherwise share one symbol and collide exactly as the bare name
/// did. Returns None when a parameter spelling has no AST
/// counterpart.
fn extern_member_symbol(name: &str, sig: &ExternSig) -> Option<String> {
    let mut tys: Vec<Type> = Vec::with_capacity(sig.params.len() + 1);
    for p in &sig.params {
        tys.push(extern_spelling_to_type(&p.ty)?);
    }
    if sig.is_variadic {
        tys.push(Type::Primitive(Primitive::VaList));
    }
    Some(sla_ast::overload_member_symbol(name, &tys))
}

/// Convert a declared parameter type back to the ABI spelling the
/// contract identity compares (Zig compares contract spellings;
/// for a source-level prototype the parser's type IS the
/// declaration, so the canonical type description is its
/// spelling). Pointer layers render as `*T`, matching the .sai
/// spelling of pointer parameters.
fn param_spelling(ty: &Type) -> String {
    match ty {
        Type::Pointer(inner) => format!("*{}", param_spelling(inner)),
        Type::Borrow(inner) => format!("&{}", param_spelling(inner)),
        // R-3: a named `args: ...` parameter's spelling is the
        // variadic marker itself; it is excluded from the fixed
        // parameter list at registration (register_source_extern).
        Type::Primitive(sla_ast::types::Primitive::VaList) => "...".to_string(),
        other => type_desc(other),
    }
}

/// Zig `primitiveFromSpelling` (staging :3204).
fn primitive_from_spelling(name: &str) -> Option<Primitive> {
    Some(match name {
        "i8" => Primitive::I8,
        "i16" => Primitive::I16,
        "i32" => Primitive::I32,
        "i64" | "isize" => Primitive::I64,
        "u8" => Primitive::U8,
        "u16" => Primitive::U16,
        "u32" => Primitive::U32,
        "u64" | "usize" => Primitive::U64,
        "f32" => Primitive::F32,
        "f64" => Primitive::F64,
        "bool" | "boolean" => Primitive::Boolean,
        "ptr" | "raw_ptr" => Primitive::RawPtr,
        "void" | "void_type" => Primitive::VoidType,
        _ => return None,
    })
}

/// Zig `contractParamMatches` (staging :3220): matching for .sai
/// contract parameters, whose types are ABI spellings. Pointer
/// spellings accept any pointer-like argument; a leading '&'
/// requires a borrow argument; primitive spellings adapt untyped
/// literals by class and otherwise demand the same primitive (the
/// untyped `integer`/`float` probe types match their whole class);
/// anything else demands the same-named user-defined type.
fn contract_param_matches(spelling_raw: &str, arg: &Node, arg_ty: &Type) -> bool {
    let spelling = spelling_raw.trim();
    // unwrapBorrowForCallArg: only an explicit borrow node unwraps.
    let effective: &Type = match (arg, arg_ty) {
        (Node::BorrowExpr(_), Type::Borrow(inner)) => inner,
        _ => arg_ty,
    };
    if spelling == "ptr" {
        return matches!(
            effective,
            Type::Pointer(_)
                | Type::Borrow(_)
                | Type::FnPtr(_)
                | Type::Primitive(Primitive::RawPtr)
        );
    }
    if spelling.starts_with('*') || spelling.starts_with('^') {
        return matches!(
            effective,
            Type::Pointer(_)
                | Type::Borrow(_)
                | Type::Primitive(Primitive::RawPtr)
        );
    }
    if spelling.starts_with('&') {
        return matches!(arg_ty, Type::Borrow(_));
    }
    if let Some(prim) = primitive_from_spelling(spelling) {
        if let Node::Literal(lit) = arg {
            match lit {
                Literal::IntVal(_) => return is_integer_type(&Type::Primitive(prim)),
                Literal::FloatVal(_) => {
                    return matches!(prim, Primitive::F32 | Primitive::F64)
                }
                _ => {}
            }
        }
        let Type::Primitive(eff) = effective else {
            return false;
        };
        if *eff == prim {
            return true;
        }
        if *eff == Primitive::Integer {
            return is_integer_type(&Type::Primitive(prim));
        }
        if *eff == Primitive::Float {
            return matches!(prim, Primitive::F32 | Primitive::F64);
        }
        return false;
    }
    match effective {
        Type::UserDefined(u) => u.name == spelling,
        _ => false,
    }
}

/// Find the member of one overload set denoted by a source-level
/// member symbol (`<name>__ov_<tags>`). Both member-key spellings
/// are accepted: this crate's `overload_member_key_for_params`
/// and the emitter's `sla_ast::overload_member_symbol` minus its
/// `sla__` prefix (the two tag schemes differ for a few types).
fn member_in_set(
    name: &str,
    set: &[FuncSig],
    symbol: &str,
) -> Option<(String, FuncSig)> {
    for sig in set {
        let params: Vec<crate::checker::ParamSig> = sig
            .params
            .iter()
            .map(|p| crate::checker::ParamSig {
                name: p.name.clone(),
                ty: p.ty.clone(),
                is_borrow: p.is_borrow,
                is_move: p.is_move,
                is_var: p.is_var,
            })
            .collect();
        if overload_member_key_for_params(name, &params) == symbol {
            return Some((name.to_string(), sig.clone()));
        }
        let tys: Vec<Type> = sig.params.iter().map(|p| p.ty.clone()).collect();
        if sla_ast::overload_member_symbol(name, &tys) == format!("sla__{symbol}") {
            return Some((name.to_string(), sig.clone()));
        }
    }
    None
}

/// The contract signature of one SOURCE-level function
/// declaration (extern prototype / definition, or a plain
/// declaration viewed under the contract discipline): the
/// declared parameter types become contract spellings. R-3: the
/// named `args: ...` va_list parameter is the variadic marker,
/// not a fixed parameter — the contract's fixed list excludes
/// it (a zero-fixed-parameter variadic routes to the header-less
/// path, whose arity must then still honor the fixed floor:
/// zero).
pub(crate) fn extern_sig_of_decl(func: &FuncDecl) -> ExternSig {
    let fixed_params: Vec<&sla_ast::decls::Param> = func
        .params
        .iter()
        .filter(|p| {
            !matches!(
                &*p.ty,
                Type::Primitive(sla_ast::types::Primitive::VaList)
            )
        })
        .collect();
    ExternSig {
        move_params: fixed_params.iter().map(|p| p.is_move).collect(),
        params: fixed_params
            .iter()
            .map(|p| ExternParam {
                name: p.name.clone(),
                ty: param_spelling(&p.ty),
                is_borrow: p.is_borrow,
                is_move: p.is_move,
                is_raw: false,
            })
            .collect(),
        is_variadic: func.is_variadic,
        ret_abi: param_spelling(&func.ret_ty),
        fallible: false,
    }
}

impl Checker {
    /// Zig `registerExternContract` for a SOURCE-level `extern "C"`
    /// declaration (prototype or `@no_mangle` definition): the
    /// declared parameter types become contract spellings and the
    /// same registration discipline applies — identical pairs
    /// dedupe silently (a definition pairs with its prototype),
    /// same parameters with a different return type are a loud
    /// Redeclaration, distinct parameters promote the name into
    /// the extern overload set.
    pub fn register_source_extern(&mut self, func: &FuncDecl) -> CheckResult<()> {
        let sig = extern_sig_of_decl(func);
        self.register_extern_contract(&func.name.clone(), sig)
    }

    /// Zig's imported overload discipline: a second DISTINCT
    /// imported signature for a name promotes it into
    /// `imported_overload_sets`; an identical re-registration
    /// dedupes silently (an imported module's declarations are
    /// inlined AND registered as signatures, so the same member
    /// arrives twice).
    pub fn register_imported_overload_member(&mut self, name: &str, sig: FuncSig) {
        if let Some(set) = self.imported_overload_sets.get_mut(name) {
            if !set
                .iter()
                .any(|s| crate::check_overload::sigs_identical(s, &sig))
            {
                set.push(sig);
            }
            return;
        }
        if let Some(existing) = self.imported_funcs.get(name).cloned() {
            if crate::check_overload::sigs_identical(&existing, &sig) {
                return;
            }
            self.imported_funcs.remove(name);
            self.imported_overload_sets
                .insert(name.to_string(), vec![existing, sig]);
            return;
        }
        self.imported_funcs.insert(name.to_string(), sig);
    }

    /// Resolve a source-level overload member symbol
    /// (`<name>__ov_<tags>`, the spelling user code writes to
    /// name one member of an overload set) to the set member it
    /// denotes, across the func and imported sets. Returns the
    /// set's base name and the member signature. Extern contract
    /// members are not resolved here (their direct member-symbol
    /// references keep the pre-existing UndefinedVariable).
    pub fn overload_member_by_symbol(&self, symbol: &str) -> Option<(String, FuncSig)> {
        for (name, set) in &self.func_overload_sets {
            if let Some(hit) = member_in_set(name, set, symbol) {
                return Some(hit);
            }
        }
        for (name, set) in &self.imported_overload_sets {
            if let Some(hit) = member_in_set(name, set, symbol) {
                return Some(hit);
            }
        }
        None
    }

    /// The unified plain-call resolution (Zig
    /// `tryResolveOverloadCall`): returns `None` when no overload
    /// set exists for the name in ANY source, leaving the legacy
    /// single-signature paths untouched. When a set exists,
    /// singleton entries from the other sources join the candidate
    /// pool so a name is never silently shadowed across sources.
    pub fn try_resolve_unified_call(
        &mut self,
        name: &str,
        call: &CallExpr,
        expr_key: usize,
    ) -> Option<CheckResult<Type>> {
        // R-3: a mixed plain + singleton-extern name (no set
        // anywhere) also resolves here — collect_candidates pools
        // singletons from every source, so the extern member
        // competes with the plain one.
        let has_set = self.func_overload_sets.contains_key(name)
            || self.imported_overload_sets.contains_key(name)
            || self.extern_overload_sets.contains_key(name)
            || (self.extern_funcs.contains_key(name)
                && self.funcs.contains_key(name));
        if !has_set {
            return None;
        }
        Some(self.resolve_unified_call(name, call, expr_key))
    }

    fn collect_candidates(&self, name: &str) -> Vec<(String, Candidate)> {
        let mut out: Vec<(String, Candidate)> = Vec::new();
        let push = |key: String, cand: Candidate, out: &mut Vec<(String, Candidate)>| {
            if !out.iter().any(|(k, _)| *k == key) {
                out.push((key, cand));
            }
        };
        if let Some(set) = self.func_overload_sets.get(name) {
            for sig in set {
                let key = overload_member_key_for_params(
                    name,
                    &sig.params
                        .iter()
                        .map(|p| crate::checker::ParamSig {
                            name: p.name.clone(),
                            ty: p.ty.clone(),
                            is_borrow: p.is_borrow,
                            is_move: p.is_move,
                            is_var: p.is_var,
                        })
                        .collect::<Vec<_>>(),
                );
                push(key, Candidate::Func(sig.clone()), &mut out);
            }
        } else if let Some(sig) = self.funcs.get(name) {
            push(name.to_string(), Candidate::Func(sig.clone()), &mut out);
        }
        if let Some(set) = self.imported_overload_sets.get(name) {
            for sig in set {
                let key = overload_member_key_for_params(
                    name,
                    &sig.params
                        .iter()
                        .map(|p| crate::checker::ParamSig {
                            name: p.name.clone(),
                            ty: p.ty.clone(),
                            is_borrow: p.is_borrow,
                            is_move: p.is_move,
                            is_var: p.is_var,
                        })
                        .collect::<Vec<_>>(),
                );
                push(key, Candidate::Imported(sig.clone()), &mut out);
            }
        } else if let Some(sig) = self.imported_funcs.get(name) {
            push(name.to_string(), Candidate::Imported(sig.clone()), &mut out);
        }
        if let Some(set) = self.extern_overload_sets.get(name) {
            for sig in set {
                let key = extern_member_key(name, sig);
                push(key, Candidate::Extern(sig.clone()), &mut out);
            }
        } else if let Some(sig) = self.extern_funcs.get(name) {
            push(name.to_string(), Candidate::Extern(sig.clone()), &mut out);
        }
        // The same signature can enter the pool twice under
        // different keys: once as an overload-set member (member
        // key) and once as a bare-inserted singleton from another
        // source (bare-name key) — e.g. an imported fixture whose
        // members were also inlined as program declarations while
        // `register_imported_signature` kept one bare entry.
        // Identical Func/Imported signatures are ONE candidate,
        // not two; without this the call is reported ambiguous
        // against itself.
        let mut seen: Vec<FuncSig> = Vec::new();
        out.retain(|(_, cand)| match cand {
            Candidate::Func(sig) | Candidate::Imported(sig) => {
                if seen
                    .iter()
                    .any(|s| crate::check_overload::sigs_identical(s, sig))
                {
                    false
                } else {
                    seen.push(sig.clone());
                    true
                }
            }
            Candidate::Extern(_) => true,
        });
        out
    }

    fn resolve_unified_call(
        &mut self,
        name: &str,
        call: &CallExpr,
        expr_key: usize,
    ) -> CheckResult<Type> {
        let candidates = self.collect_candidates(name);
        // A generic template must stand alone (see check_overload.rs).
        if self
            .func_generics
            .get(name)
            .map(|g| !g.is_empty())
            .unwrap_or(false)
        {
            return Err(Diagnostic::new(
                ErrorKind::GenericOverloadConflict,
                format!("Monomorphization Error: generic function `{name}` shares its name with another function declaration; generic templates do not participate in overloading, so a name with a generic template must stand alone"),
            ));
        }
        // Probe argument types under a snapshot; probing must not
        // consume (Zig rolls the probe states back before the
        // winning member's real argument check).
        let saved = self.scopes.save_states();
        let mut arg_tys: Vec<Type> = Vec::new();
        let mut probe_error: Option<Diagnostic> = None;
        for arg in &call.args {
            match self.check_expr(arg) {
                Ok(ty) => arg_tys.push(ty),
                Err(e) => {
                    probe_error = Some(e);
                    break;
                }
            }
        }
        self.scopes.restore_states(&saved);
        if let Some(e) = probe_error {
            return Err(e);
        }
        let mut winners: Vec<(&String, &Candidate)> = Vec::new();
        for (key, cand) in &candidates {
            let matched = match cand {
                Candidate::Func(sig) | Candidate::Imported(sig) => {
                    // R-3: a variadic member matches at or above
                    // its fixed parameter count (trailing args are
                    // matched only over the fixed prefix).
                    (if sig.is_variadic {
                        call.args.len() >= sig.params.len()
                    } else {
                        sig.params.len() == call.args.len()
                    })
                        && sig
                            .params
                            .iter()
                            .zip(call.args.iter())
                            .zip(arg_tys.iter())
                            .all(|((p, a), t)| {
                                crate::check_overload::overload_param_matches(p, a, t)
                            })
                }
                Candidate::Extern(sig) => {
                    let arity_ok = if sig.is_variadic {
                        call.args.len() >= sig.params.len()
                    } else {
                        call.args.len() == sig.params.len()
                    };
                    arity_ok
                        && sig
                            .params
                            .iter()
                            .zip(call.args.iter())
                            .zip(arg_tys.iter())
                            .all(|((p, a), t)| {
                                if p.is_move && !matches!(a, Node::MoveExpr(_)) {
                                    return false;
                                }
                                contract_param_matches(&p.ty, a, t)
                            })
                }
            };
            if matched {
                winners.push((key, cand));
            }
        }
        match winners.len() {
            0 => Err(Diagnostic::new(
                ErrorKind::TypeMismatch,
                format!(
                    "no overload of `{name}` matches the argument types ({}); candidates: {}",
                    arg_tys
                        .iter()
                        .map(|t| type_desc(t))
                        .collect::<Vec<_>>()
                        .join(", "),
                    candidates
                        .iter()
                        .map(|(_, c)| c.render(name))
                        .collect::<Vec<_>>()
                        .join("; ")
                ),
            )),
            1 => match winners[0].1 {
                Candidate::Func(sig) | Candidate::Imported(sig) => {
                    let sig = sig.clone();
                    // Record the winning member's mangled symbol
                    // so the text emitter binds this call to that
                    // member's definition (mirrors
                    // resolve_overload_set_call, check_overload.rs).
                    // Pool-size gate (R544): the pool holds every
                    // distinct member across sources, so
                    // `candidates.len() > 1` is exactly "this name
                    // has more than one member", matching the
                    // emitter's definition-count mangling in
                    // `decl_symbol`. The old winner-key gate
                    // (`winners[0].0 != name`) misread mixed
                    // plain+extern sets, where the extern singleton
                    // carries the bare-name key. A true singleton
                    // (pool of one) keeps its bare-name binding.
                    if candidates.len() > 1 {
                        let tys: Vec<Type> =
                            sig.params.iter().map(|p| p.ty.clone()).collect();
                        self.overload_call_symbols.insert(
                            expr_key,
                            sla_ast::overload_member_symbol(name, &tys),
                        );
                    }
                    self.check_call_args_against_sig(name, &sig, &call.args, &call.generics)
                }
                Candidate::Extern(sig) => {
                    let sig = sig.clone();
                    // Record the winning member's mangled symbol
                    // for a set member (Root-3 follow-up — mirrors
                    // the Func/Imported arm above): without it the
                    // text emitter binds the call to the bare set
                    // name, whose member definitions now carry
                    // member symbols. Pool-size gate (R544, as
                    // above): a true singleton (pool of one) keeps
                    // its bare-name binding; an unconvertible
                    // parameter spelling likewise skips recording
                    // (today's behaviour) rather than recording a
                    // symbol the emitter cannot match.
                    if candidates.len() > 1 {
                        if let Some(sym) = extern_member_symbol(name, &sig) {
                            self.overload_call_symbols.insert(expr_key, sym);
                        }
                    }
                    self.check_extern_contract_call(name, &sig, call)
                }
            },
            _ => Err(Diagnostic::new(
                ErrorKind::AmbiguousCall,
                format!(
                    "AmbiguousCall: call to `{name}` matches multiple overloads: {}",
                    winners
                        .iter()
                        .map(|(_, c)| c.render(name))
                        .collect::<Vec<_>>()
                        .join("; ")
                ),
            )),
        }
    }


}
