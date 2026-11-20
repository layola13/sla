//! Method / associated-call resolution machinery: the receiver
//! type derivation paths of `type_checker.zig` (line numbers are
//! the venue tree at head 6002b386, 10,917 lines; the local
//! 6,098-line tree carries the same machinery at lower offsets).
//!
//! Forms mirrored here (CHK3, phase 3):
//!   * associated trait form `Trait::method(recv, ...)` (:8438):
//!     the receiver is args[0]; its concrete type selects the impl;
//!   * associated inherent `{Target}_{method}` (:8787) and the
//!     associated trait fallback for a type target (:8796);
//!   * receiver-inferred inherent (:9669): a plain call whose
//!     args[0] peels (borrow/pointer) to a user-defined type `T`
//!     resolves `{T}_{method}` in funcs, then imported signatures,
//!     with `auto_borrow_receiver = true`;
//!   * receiver-inferred trait (:9688): the same call resolves a
//!     trait impl symbol `{T}__{Trait}_{method}` via
//!     `resolveTraitMethodSymbolForType` (ambiguity = TypeMismatch);
//!   * dyn dispatch: a `__dyn_<Trait>` receiver resolves through
//!     the trait's declared method (params[1..] checked);
//!   * `using` static extensions (:9703-9725, resolver :2922,
//!     call checker :2978): AFTER inherent and trait, only for
//!     non-associated calls whose args[0] is not a move expr and
//!     only when a `using` module is in scope; only pub module
//!     functions register; ambiguity = AmbiguousCall.
//!
//! The shared argument checker is Zig
//! `checkCallArgsAgainstSignature` (venue :1240s), ported as
//! [`Checker::check_method_args`] and used ONLY by the method
//! paths above; the plain-call path keeps its earlier simplified
//! checker (check_call.rs) — see D-C21 in
//! docs/divergences/checker.md.
//!
//! The borrow-array-to-slice coercion and its
//! `array_to_slice_borrow_args` side table ARE here (ported by
//! LINE-ARRAY2SLICE, closing that part of D-C21): see
//! `plain_call_arg_matches`. The name-keyed std
//! leaf cascade and the static-constructor leaves that used
//! to be listed here were ported by the R-1 line into
//! check_leaves.rs / check_receiver_leaves.rs (+2).

use crate::checker::{Checker, FuncSig, ParamSig};
use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use crate::types_util::{
    box_inner_type, can_coerce_borrow_array_to_borrow_slice,
    concrete_type_name, dyn_trait_name, is_numeric_type, is_string_type,
    node_key_of, peel_borrow_pointer, rc_inner_type, type_desc,
    type_tag_name, types_equal, types_structurally_equal,
};
use sla_ast::exprs::{CallExpr, Literal};
use sla_ast::node::Node;
use sla_ast::types::Type;

impl Checker {
    // -- trait symbol resolution ------------------------------------

    /// Mirrors Zig `resolveTraitMethodSymbolForType` (venue
    /// :5849): with a target trait, the type must implement it
    /// (directly or through an extending impl), the DECLARING
    /// trait in its hierarchy names the symbol, and the symbol's
    /// parameter count (funcs or imported signatures) must equal
    /// the call's argument count. Without a target trait, every
    /// registered impl of the type is scanned; two DISTINCT
    /// symbols are an ambiguity (TypeMismatch, Zig message kept).
    /// Zig iterates a hash map here; the scan below collects and
    /// sorts candidates so the outcome is order-independent —
    /// the ambiguity rule already makes multi-symbol outcomes
    /// deterministic in Zig.
    pub fn resolve_trait_method_symbol(
        &self,
        type_name: &str,
        target_trait: Option<&str>,
        method_name: &str,
        arg_count: usize,
    ) -> CheckResult<Option<String>> {
        if let Some(trait_name) = target_trait {
            if !self.type_directly_implements_trait(type_name, trait_name)
                && !self.type_implements_trait_name(type_name, trait_name)
            {
                return Ok(None);
            }
            let declaring = match self
                .find_trait_declaring_method(trait_name, method_name)
            {
                Some(d) => d,
                None => return Ok(None),
            };
            let symbol = format!("{type_name}__{declaring}_{method_name}");
            let count = match self.signature_param_count(&symbol) {
                Some(c) => c,
                None => return Ok(None),
            };
            if count != arg_count {
                return Ok(None);
            }
            return Ok(Some(symbol));
        }

        let mut symbols: Vec<String> = Vec::new();
        for key in &self.trait_impls {
            let Some((impl_trait, impl_type)) = key.split_once('|') else {
                continue;
            };
            if impl_type != type_name {
                continue;
            }
            let declaring = match self
                .find_trait_declaring_method(impl_trait, method_name)
            {
                Some(d) => d,
                None => continue,
            };
            let symbol = format!("{type_name}__{declaring}_{method_name}");
            let count = match self.signature_param_count(&symbol) {
                Some(c) => c,
                None => continue,
            };
            if count != arg_count {
                continue;
            }
            if !symbols.contains(&symbol) {
                symbols.push(symbol);
            }
        }
        symbols.sort();
        match symbols.len() {
            0 => Ok(None),
            1 => Ok(Some(symbols.pop().expect("len checked"))),
            _ => Err(Diagnostic::new(
                ErrorKind::TypeMismatch,
                format!(
                    "Ambiguous trait method `{method_name}` for type `{type_name}`"
                ),
            )),
        }
    }

    /// Zig `functionOrImportedSignatureParamCount`.
    fn signature_param_count(&self, symbol: &str) -> Option<usize> {
        if let Some(sig) = self.funcs.get(symbol) {
            return Some(sig.params.len());
        }
        self.imported_funcs.get(symbol).map(|s| s.params.len())
    }

    // -- using extension resolution ---------------------------------

    /// Mirrors Zig `receiverParamMatches` (venue :1230s): a borrow
    /// parameter compares against the UNWRAPPED receiver; a plain
    /// parameter accepts the receiver type or its unwrapping.
    fn receiver_param_matches(&self, param: &ParamSig, recv_ty: &Type) -> bool {
        let recv = peel_borrow_pointer(recv_ty);
        if param.is_borrow {
            return types_equal(&param.ty, recv);
        }
        types_equal(&param.ty, recv_ty) || types_equal(&param.ty, recv)
    }

    /// Mirrors Zig `methodForType` (type_checker.zig:3322): the
    /// receiver type is unwrapped through borrow/pointer layers
    /// and must be user-defined; the method is the `funcs` entry
    /// keyed `{type}_{method}`.
    pub(crate) fn method_for_type(
        &self,
        ty: &Type,
        method_name: &str,
    ) -> Option<FuncSig> {
        let recv = peel_borrow_pointer(ty);
        let Type::UserDefined(u) = recv else {
            return None;
        };
        self.funcs.get(&format!("{}_{}", u.name, method_name)).cloned()
    }

    /// Mirrors Zig `protocolIterableElementType`
    /// (type_checker.zig:4714): a type is iterable by protocol
    /// when it offers `iter_len` (one receiver parameter, numeric
    /// return) and `iter_at` (receiver plus one numeric index
    /// parameter); the element type is `iter_at`'s return type.
    /// Both receiver parameters must match the iterated type
    /// under the usual receiver rules.
    pub(crate) fn protocol_iterable_element_type(
        &self,
        ty: &Type,
    ) -> Option<Type> {
        let len_func = self.method_for_type(ty, "iter_len")?;
        let at_func = self.method_for_type(ty, "iter_at")?;
        if len_func.params.len() != 1 || at_func.params.len() != 2 {
            return None;
        }
        if !self.receiver_param_matches(&len_func.params[0], ty) {
            return None;
        }
        if !self.receiver_param_matches(&at_func.params[0], ty) {
            return None;
        }
        if !is_numeric_type(&len_func.ret_ty) {
            return None;
        }
        if !is_numeric_type(&at_func.params[1].ty) {
            return None;
        }
        Some(at_func.ret_ty.clone())
    }

    /// Mirrors Zig `extensionReceiverParamMatches`: the general
    /// match, plus a first parameter declared with a borrow TYPE
    /// matches an owned or borrowed receiver of the inner type.
    fn extension_receiver_param_matches(
        &self,
        param: &ParamSig,
        recv_ty: &Type,
    ) -> bool {
        if self.receiver_param_matches(param, recv_ty) {
            return true;
        }
        if let Type::Borrow(inner) = &param.ty {
            return types_equal(inner, peel_borrow_pointer(recv_ty));
        }
        false
    }

    /// Mirrors Zig `resolveUsingMethodSymbolForType` (venue
    /// :2922): for each in-scope `using` module (innermost first),
    /// the symbol `{module}__{method}` must exist in funcs, be
    /// pub, match arity, and match the receiver. Two distinct
    /// symbols are an AmbiguousCall (message kept verbatim; the
    /// receiver name is the user-defined name or the type's tag
    /// name — venue probe: an i32 receiver reports `primitive`).
    pub fn resolve_using_method_symbol(
        &self,
        recv_ty: &Type,
        method_name: &str,
        arg_count: usize,
    ) -> CheckResult<Option<String>> {
        let mut found: Option<String> = None;
        for module in self.current_using_modules() {
            let symbol = format!("{module}__{method_name}");
            let Some(func) = self.funcs.get(&symbol) else {
                continue;
            };
            if !func.is_pub {
                continue;
            }
            if func.params.len() != arg_count {
                continue;
            }
            let Some(first) = func.params.first() else {
                continue;
            };
            if !self.extension_receiver_param_matches(first, recv_ty) {
                continue;
            }
            if let Some(prev) = &found {
                if prev != &symbol {
                    let unwrapped = peel_borrow_pointer(recv_ty);
                    let recv_name = match unwrapped {
                        Type::UserDefined(u) => u.name.clone(),
                        other => type_tag_name(other).to_string(),
                    };
                    return Err(Diagnostic::new(
                        ErrorKind::AmbiguousCall,
                        format!(
                            "Ambiguous static extension call `{method_name}` for type `{recv_name}`"
                        ),
                    ));
                }
            } else {
                found = Some(symbol);
            }
        }
        Ok(found)
    }

    /// Mirrors Zig `checkUsingExtensionCall` (venue :2978): when
    /// the extension's first parameter is declared with a borrow
    /// TYPE and the receiver argument is not already a borrow
    /// expression, the parameter is re-expressed in the `&self`
    /// flag form so argument checking applies the method-call
    /// auto-borrow; the callee signature itself is unchanged.
    pub fn check_using_extension_call(
        &mut self,
        symbol: &str,
        call: &CallExpr,
    ) -> CheckResult<Type> {
        let sig = self.funcs.get(symbol).cloned().ok_or_else(|| {
            Diagnostic::new(
                ErrorKind::UndefinedVariable,
                format!("UndefinedVariable: `{symbol}`"),
            )
        })?;
        if let Some(first) = sig.params.first() {
            if let Type::Borrow(inner) = &first.ty {
                if !first.is_borrow
                    && !matches!(call.args.first(), Some(Node::BorrowExpr(_)))
                {
                    let mut adjusted = sig.clone();
                    adjusted.params[0].is_borrow = true;
                    adjusted.params[0].ty = (**inner).clone();
                    return self.check_method_args(
                        &call.func_name,
                        &adjusted,
                        &call.args,
                        true,
                    );
                }
            }
        }
        self.check_method_args(&call.func_name, &sig, &call.args, true)
    }

    // -- the shared signature checker (method paths) ----------------

    /// Mirrors Zig `plainCallArgMatches`: the unwrapped-borrow
    /// equality, the string-literal-to-String case, the
    /// borrow-array-to-slice coercion (which also records the
    /// argument in `array_to_slice_borrow_args`, like Zig), and
    /// the borrow-expression-to-pointer-parameter case.
    fn plain_call_arg_matches(
        &mut self,
        param_ty: &Type,
        arg: &Node,
        arg_ty: &Type,
    ) -> bool {
        if types_equal(param_ty, &unwrap_borrow_for_call_arg(arg, arg_ty)) {
            return true;
        }
        if matches!(arg, Node::Literal(Literal::StringVal(_)))
            && is_string_type(param_ty)
        {
            return true;
        }
        if matches!(arg, Node::BorrowExpr(_))
            && can_coerce_borrow_array_to_borrow_slice(param_ty, arg_ty)
        {
            self.record_array_to_slice_borrow_arg(arg);
            return true;
        }
        if !matches!(arg, Node::BorrowExpr(_)) {
            return false;
        }
        let Type::Borrow(borrow_inner) = arg_ty else {
            return false;
        };
        match param_ty {
            Type::Pointer(inner) => types_equal(inner, borrow_inner),
            _ => false,
        }
    }

    /// Mirrors Zig `checkCallArgsAgainstSignature` (venue :1240s)
    /// for the method / associated / extension paths, including
    /// the receiver auto-borrow branch and the width-strict array
    /// element rule. The dyn-borrow parameter branch is ported in
    /// its verdict-relevant part (trait extension / implementation
    /// checks); Zig's `dyn_borrow_args` side table has no analogue
    /// in this crate and is not recorded (D-C21).
    pub fn check_method_args(
        &mut self,
        name: &str,
        sig: &FuncSig,
        args: &[Node],
        auto_borrow_receiver: bool,
    ) -> CheckResult<Type> {
        if sig.params.len() != args.len() {
            return Err(Diagnostic::new(
                ErrorKind::InvalidArgsCount,
                format!(
                    "InvalidArgsCount: `{name}` expects {} args, got {}",
                    sig.params.len(),
                    args.len()
                ),
            ));
        }
        // View-types stage 1 (LINE-VIEWARGS-FIX): method calls go
        // through the same Zig `checkCallArgsAgainstSignature`
        // place-overlap gate as plain calls (the receiver is
        // args[0] / params[0] here).
        self.check_call_arg_place_overlaps(
            name,
            &sig.params,
            &sig.param_may_write,
            args,
        )?;
        for (i, (param, arg)) in sig.params.iter().zip(args.iter()).enumerate()
        {
            if param.is_move && !matches!(arg, Node::MoveExpr(_)) {
                return Err(Diagnostic::new(
                    ErrorKind::TypeMismatch,
                    format!(
                        "TypeMismatch: parameter `{}` of `{name}` requires a move argument",
                        param.name
                    ),
                ));
            }
            if param.is_borrow {
                if let Some(trait_name) = dyn_trait_name(&param.ty) {
                    let trait_name = trait_name.to_string();
                    let arg_ty = self.check_expr(arg)?;
                    if let Some(arg_trait) = dyn_trait_name(&arg_ty) {
                        if !self.trait_extends_trait(arg_trait, &trait_name) {
                            return Err(Diagnostic::new(
                                ErrorKind::TypeMismatch,
                                format!(
                                    "TypeMismatch: dyn argument does not extend trait `{trait_name}`"
                                ),
                            ));
                        }
                        continue;
                    }
                    let concrete = match &arg_ty {
                        Type::Borrow(inner) => (**inner).clone(),
                        _ => {
                            return Err(Diagnostic::new(
                                ErrorKind::TypeMismatch,
                                "TypeMismatch: dyn parameter requires a borrow argument",
                            ))
                        }
                    };
                    if !self.type_implements_trait(&concrete, &trait_name) {
                        return Err(Diagnostic::new(
                            ErrorKind::TypeMismatch,
                            format!(
                                "TypeMismatch: type does not implement trait `{trait_name}`"
                            ),
                        ));
                    }
                    continue;
                }
            }
            if auto_borrow_receiver
                && i == 0
                && param.is_borrow
                && !matches!(arg, Node::BorrowExpr(_))
            {
                let arg_ty = self.check_expr(arg)?;
                let effective = match &arg_ty {
                    Type::Borrow(inner) => (**inner).clone(),
                    other => other.clone(),
                };
                if let Some(target_trait) = dyn_trait_name(&param.ty) {
                    if let Some(arg_trait) = dyn_trait_name(&effective) {
                        if !self.trait_extends_trait(arg_trait, target_trait) {
                            return Err(Diagnostic::new(
                                ErrorKind::TypeMismatch,
                                "TypeMismatch: dyn receiver does not extend the target trait",
                            ));
                        }
                        continue;
                    }
                }
                if !types_equal(&param.ty, &effective) {
                    return Err(Diagnostic::new(
                        ErrorKind::TypeMismatch,
                        format!(
                            "TypeMismatch: receiver argument for `{name}` expects {}, actual {}",
                            type_desc(&param.ty),
                            type_desc(&effective)
                        ),
                    ));
                }
                continue;
            }
            if let Type::Borrow(param_inner) = &param.ty {
                let arg_ty = self.check_expr(arg)?;
                let matched = match &arg_ty {
                    Type::Borrow(arg_inner) => {
                        types_equal(param_inner, arg_inner)
                            // Same reborrow acceptance as the
                            // plain-call path (check_call_sig.rs):
                            // Zig's one shared plainCallArgMatches
                            // unwraps a borrow expression argument.
                            || self.plain_call_arg_matches(
                                &param.ty, arg, &arg_ty,
                            )
                    }
                    _ => false,
                };
                if !matched {
                    return Err(Diagnostic::new(
                        ErrorKind::TypeMismatch,
                        format!(
                            "TypeMismatch: argument for parameter `{}` of `{name}` must be a borrow",
                            param.name
                        ),
                    ));
                }
                continue;
            }
            if param.is_borrow && !matches!(arg, Node::BorrowExpr(_)) {
                let arg_ty = self.check_expr(arg)?;
                let matched = match &arg_ty {
                    Type::Borrow(inner) => types_equal(&param.ty, inner),
                    _ => false,
                };
                if !matched {
                    return Err(Diagnostic::new(
                        ErrorKind::TypeMismatch,
                        format!(
                            "TypeMismatch: parameter `{}` of `{name}` requires a borrow argument",
                            param.name
                        ),
                    ));
                }
                continue;
            }
            if !param.is_move
                && !param.is_borrow
                && (matches!(arg, Node::MoveExpr(_))
                    || (matches!(arg, Node::BorrowExpr(_))
                        && !matches!(param.ty, Type::Pointer(_))))
            {
                return Err(Diagnostic::new(
                    ErrorKind::TypeMismatch,
                    format!(
                        "Call to {name} passes capability argument to plain parameter {}",
                        param.name
                    ),
                ));
            }
            let arg_ty = self.check_expr(arg)?;
            if !self.plain_call_arg_matches(&param.ty, arg, &arg_ty) {
                return Err(Diagnostic::new(
                    ErrorKind::TypeMismatch,
                    format!(
                        "TypeMismatch: argument for parameter `{}` of `{name}` expects {}, actual {}",
                        param.name,
                        type_desc(&param.ty),
                        type_desc(&arg_ty)
                    ),
                ));
            }
            if let (Type::Array(pa), Type::Array(aa)) = (&param.ty, &arg_ty) {
                // R2b (stage 6): a literal argument takes its
                // element width from the parameter (recorded
                // below); the width-strict rule governs real
                // values, whose width is their own.
                let literal_arg = matches!(
                    arg,
                    Node::ArrayLiteral(_) | Node::RepeatArrayLiteral(_)
                );
                if pa.len != aa.len
                    || (!literal_arg
                        && !types_structurally_equal(&pa.elem, &aa.elem))
                {
                    return Err(Diagnostic::new(
                        ErrorKind::TypeMismatch,
                        format!(
                            "TypeMismatch in call to {name}: array element type mismatch (stride differs)"
                        ),
                    ));
                }
            }
            if matches!(
                arg,
                Node::TupleLiteral(_)
                    | Node::ArrayLiteral(_)
                    | Node::RepeatArrayLiteral(_)
            ) && types_equal(&arg_ty, &param.ty)
            {
                self.expr_types
                    .insert(node_key_of(arg), param.ty.clone());
            }
        }
        Ok(sig.ret_ty.clone())
    }

    // -- receiver-inferred resolution --------------------------------

    /// The receiver-inferred block (venue :9669-9725 plus the dyn
    /// arm of the receiver cascade): inherent, then trait, then
    /// dyn, then `using` extensions — Zig's fixed order per the
    /// DEF-E0 comment at :9703. Returns `None` when no form
    /// resolves, leaving the caller's fallback to fire.
    pub fn check_receiver_call(
        &mut self,
        call: &CallExpr,
    ) -> Option<CheckResult<Type>> {
        if call.args.is_empty() {
            return None;
        }
        let name = call.func_name.as_str();
        // Zig re-checks args[0] at every resolution stage; an error
        // typing the receiver is the call's error.
        let recv_ty = match self.check_expr(&call.args[0]) {
            Ok(t) => t,
            Err(e) => return Some(Err(e)),
        };

        // 1. Inherent: peel borrow/pointer to a user-defined type.
        if let Type::UserDefined(u) = peel_borrow_pointer(&recv_ty) {
            let key = format!("{}_{name}", u.name);
            let sig = self
                .funcs
                .get(&key)
                .cloned()
                .or_else(|| self.imported_funcs.get(&key).cloned());
            if let Some(sig) = sig {
                return Some(self.check_method_args(
                    name,
                    &sig,
                    &call.args,
                    true,
                ));
            }
        }

        // 2. Trait: concrete type name (after the same peel).
        if let Some(type_name) = concrete_type_name(&recv_ty) {
            let type_name = type_name.to_string();
            match self.resolve_trait_method_symbol(
                &type_name,
                None,
                name,
                call.args.len(),
            ) {
                Err(e) => return Some(Err(e)),
                Ok(Some(symbol)) => {
                    let sig = self
                        .funcs
                        .get(&symbol)
                        .cloned()
                        .or_else(|| self.imported_funcs.get(&symbol).cloned());
                    return Some(match sig {
                        Some(sig) => self.check_method_args(
                            name,
                            &sig,
                            &call.args,
                            true,
                        ),
                        None => Err(Diagnostic::new(
                            ErrorKind::UndefinedVariable,
                            format!("UndefinedVariable: `{symbol}`"),
                        )),
                    });
                }
                Ok(None) => {}
            }
        }

        // 3. Dyn dispatch: a `__dyn_<Trait>` receiver; the trait's
        // declared method supplies the signature, the receiver
        // parameter itself is not re-checked (Zig zips [1..]).
        // F-DYN L1: a smart-pointer receiver (`Rc<__dyn_T>` /
        // `Box<__dyn_T>`) retries the dyn step against the
        // wrapper's single generic argument — the same unwrap the
        // text emitter's `dyn_receiver_trait` performs
        // (dyn_coercion.rs). Without it the step stripped the
        // `__dyn_` prefix from the wrapper's own name (`Rc` /
        // `Box`) and missed, so every direct dyn call on a
        // smart-pointer receiver fell through to `Undefined
        // call`. Arc is deliberately not unwrapped: the emitter
        // has no Arc dyn-dispatch plan, so accepting the receiver
        // here would trade this checker verdict for a codegen
        // failure.
        let dyn_name = dyn_trait_name(&recv_ty)
            .map(str::to_string)
            .or_else(|| {
                rc_inner_type(&recv_ty)
                    .or_else(|| box_inner_type(&recv_ty))
                    .and_then(|inner| {
                        dyn_trait_name(&inner).map(str::to_string)
                    })
            });
        if let Some(trait_name) = dyn_name {
            if let Some(method) = self.find_trait_method(&trait_name, name) {
                if method.params.len() != call.args.len() {
                    return Some(Err(Diagnostic::new(
                        ErrorKind::InvalidArgsCount,
                        format!(
                            "InvalidArgsCount: dyn method `{name}` expects {} args, got {}",
                            method.params.len(),
                            call.args.len()
                        ),
                    )));
                }
                let result = (|| -> CheckResult<Type> {
                    for (param, arg) in method
                        .params
                        .iter()
                        .skip(1)
                        .zip(call.args.iter().skip(1))
                    {
                        let arg_ty = self.check_expr(arg)?;
                        if !types_equal(
                            &param.ty,
                            &unwrap_borrow_for_call_arg(arg, &arg_ty),
                        ) {
                            return Err(Diagnostic::new(
                                ErrorKind::TypeMismatch,
                                format!(
                                    "TypeMismatch: dyn method `{name}` argument mismatch"
                                ),
                            ));
                        }
                    }
                    Ok(method.ret_ty.clone())
                })();
                return Some(result);
            }
        }

        // 4. `using` static extensions (venue conditions at
        // :9711: non-associated call, args[0] not a move expr, a
        // `using` module in scope).
        if call.associated_target.is_none()
            && !matches!(&call.args[0], Node::MoveExpr(_))
            && !self.current_using_modules().is_empty()
        {
            match self.resolve_using_method_symbol(
                &recv_ty,
                name,
                call.args.len(),
            ) {
                Err(e) => return Some(Err(e)),
                Ok(Some(symbol)) => {
                    return Some(
                        self.check_using_extension_call(&symbol, call),
                    )
                }
                Ok(None) => {}
            }
        }

        // 5. Std receiver leaves (R-2 port, check_std_leaf.rs /
        // check_std_leaf2.rs / check_std_leaf3.rs): the
        // receiver-shape cascade and the name-keyed container
        // arms of the Zig call arm. They run last inside this
        // dispatch — inherent / trait / dyn / `using` keep
        // precedence, and the R-1 receiver cascade
        // (check_receiver_leaves.rs, consulted from check_call
        // before this function) has already had first refusal
        // on every shared name, so current-head verdicts win;
        // a leaf here can only resolve a call every earlier
        // stage missed.
        if let Some(result) = self.check_std_receiver_leaf(call, &recv_ty) {
            return Some(result);
        }
        if let Some(result) = self.check_std_receiver_leaf2(call, &recv_ty) {
            return Some(result);
        }

        None
    }
}

/// Zig `unwrapBorrowForCallArg`: a borrow EXPRESSION unwraps to
/// its inner type; every other argument keeps its checked type
/// (in particular, an identifier whose TYPE is a borrow does NOT
/// unwrap — venue probes a_borrow_recv / a_ptr_recv pin this).
/// Crate-level view of `plain_call_arg_matches` for the plain-call
/// path (check_call.rs): Zig has ONE plainCallArgMatches shared by
/// both signature checkers; the plain path consults it only from the
/// borrow-typed-parameter arm, matching that arm's Zig verdicts.
pub fn plain_call_arg_matches_pub(
    checker: &mut Checker,
    param_ty: &Type,
    arg: &Node,
    arg_ty: &Type,
) -> bool {
    checker.plain_call_arg_matches(param_ty, arg, arg_ty)
}

pub fn unwrap_borrow_for_call_arg(arg: &Node, arg_ty: &Type) -> Type {
    if matches!(arg, Node::BorrowExpr(_)) {
        if let Type::Borrow(inner) = arg_ty {
            return (**inner).clone();
        }
    }
    arg_ty.clone()
}
