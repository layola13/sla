//! R-3: arena domain checking — the `arena { ... }` block statement
//! and the `@arena fn` sugar (F2-A C1 design, Jev U1 = C block form +
//! attribute sugar; U2 = A v1 banned classes; U3 = A clone-only
//! export; U4 = A nested blocks are independent domains).
//!
//! The design postdates the Zig tree this port mirrors (the local
//! `sa_plugin_sla` has no arena surface at all), so the corpus files
//! (`test_error_arena_*` / `test_arena_ok_*`, whose `// expect:` lines
//! and design-section comments are the authority) define the gates:
//!
//! * **ArenaUnsafe** — a root's type closure reaches a banned class:
//!   reference-counted (Arc/Rc/Weak), independent heap (Box),
//!   self-managed containers (HashMap/BTreeMap/HashSet/BTreeSet/
//!   VecDeque/StringBuf), handles (File/Tcp*/Chan/Mutex/RwLock/
//!   OnceLock/RefCell/Sender/Receiver), raw pointers, borrow fields,
//!   unions, opaque structs, and undeclared types (closed world).
//!   The closure substitutes generics and never cuts recursion
//!   (design 2.4: greatest fixpoint still reaches through cycles).
//! * **ArenaEscape** — a root leaves its domain: returned (directly
//!   or reachable through a literal / a closure that captured it),
//!   stored into an outer-domain place, or passed by value to an
//!   ordinary function. Clone-out (borrow in, fresh value out) is the
//!   sanctioned export and stays legal.
//! * **ArenaDomain** — an ordinary-domain (or outer-arena) deep value
//!   is moved INTO an arena binding (design 3 R3 / R6).
//! * **ArenaGoroutine** — a root crosses the goroutine boundary:
//!   sent on a channel or passed to a `go` spawn (design 4).
//!
//! State lives in a thread-local stack: `checker.rs` is at the
//! 700-line cap, and arena checking is a single-threaded pass, so a
//! scoped stack beside the checker keeps the wiring to a few lines.
//! The stack is always empty outside arena checking.

use std::cell::RefCell;
use std::collections::HashSet;

use sla_ast::exprs::CallExpr;
use sla_ast::node::Node;
use sla_ast::types::Type;

use crate::checker::Checker;
use crate::diag::{CheckResult, Diagnostic, ErrorKind};

/// One live arena domain.
pub struct ArenaFrame {
    /// Scope-frame index of the arena block's own scope. Bindings
    /// defined in exactly this frame are this domain's roots.
    pub frame: usize,
    /// Root names defined directly in this domain so far.
    pub roots: Vec<String>,
    /// Bindings defined in this domain whose value is a closure that
    /// captured one of this domain's roots (escape via return).
    pub tainted_closures: Vec<String>,
}

thread_local! {
    static ARENA_STACK: RefCell<Vec<ArenaFrame>> = const { RefCell::new(Vec::new()) };
}

/// Push a domain whose roots live in scope frame `frame`
/// (used by `check_arena_block` and the `@arena fn` path).
pub fn push_arena_frame(frame: usize) {
    with_stack(|s| {
        s.push(ArenaFrame {
            frame,
            roots: Vec::new(),
            tainted_closures: Vec::new(),
        })
    });
}

/// Pop the innermost domain.
pub fn pop_arena_frame() {
    with_stack(|s| {
        s.pop();
    });
}

fn with_stack<R>(f: impl FnOnce(&mut Vec<ArenaFrame>) -> R) -> R {
    ARENA_STACK.with(|s| f(&mut s.borrow_mut()))
}

/// Whether any arena domain is currently live.
pub fn arena_active() -> bool {
    with_stack(|s| !s.is_empty())
}

fn err(kind: ErrorKind, msg: String) -> Diagnostic {
    Diagnostic::new(kind, msg)
}

const BANNED: &[&str] = &[
    "Arc", "Rc", "Weak", "Box", "RefCell", "Mutex", "RwLock", "OnceLock", "HashMap", "BTreeMap",
    "HashSet", "BTreeSet", "VecDeque", "StringBuf", "File", "TcpStream", "TcpListener",
    "UdpSocket", "Chan", "Sender", "Receiver",
];

/// Flat-world leaf types that need no declaration lookup.
const SAFE_LEAVES: &[&str] = &[
    "String", "Vec", "GrowVec", "Option", "Result", "Self",
];

impl Checker {
    /// Check an `arena { ... }` block (or an `@arena fn` body) as one
    /// arena domain: push a frame, check the body, pop.
    pub fn check_arena_block(&mut self, body: &[Node]) -> CheckResult<()> {
        // `check_block`'s push returns the new frame's index (it
        // appends to the frames vec): depth() is that index exactly.
        let frame = self.scopes.depth();
        push_arena_frame(frame);
        let result = self.check_block(body);
        pop_arena_frame();
        result
    }

    /// Record a freshly defined binding (from `check_let`) with the
    /// arena gates: root safety for this domain's own bindings,
    /// move-in rejection for outer deep values, closure taint.
    pub fn arena_on_let(
        &mut self,
        name: &str,
        ty: &Type,
        value: &Node,
    ) -> CheckResult<()> {
        if !arena_active() {
            return Ok(());
        }
        // Domain gate FIRST (design 3 R3 / R6): a let whose value
        // moves a non-Copy, non-borrow value in from outside the
        // CURRENT (innermost) arena domain crosses domains — whether
        // the target lands in the arena frame itself (move_in) or in
        // a nested arena (nested_cross). A bare identifier or an
        // explicit `^` move wrapper is a move source; a root of the
        // current domain is same-domain (its consumption is the
        // ordinary let-move's business). This must precede root
        // registration: "defined in the arena frame" does not make a
        // moved-in outer value an arena root.
        let move_src = match value {
            Node::Identifier(src) => Some(src.as_str()),
            Node::MoveExpr(m) => match &*m.expr {
                Node::Identifier(src) => Some(src.as_str()),
                _ => None,
            },
            _ => None,
        };
        if let Some(src) = move_src {
            if !self.is_current_root(src) {
                if let Some(t) = self.lookup_ty(src) {
                    if !self.type_is_copy(&t)
                        && !crate::types_util::is_borrow_like_type(&t)
                    {
                        return Err(err(
                            ErrorKind::ArenaDomain,
                            format!(
                                "ArenaDomain: moving ordinary-domain value `{src}` into an arena binding crosses domains; clone it explicitly"
                            ),
                        ));
                    }
                }
            }
        }
        let frame_of = self.scopes.frame_of(name);
        let is_root = with_stack(|s| {
            s.last().map(|f| Some(f.frame) == frame_of).unwrap_or(false)
        });
        if is_root {
            // `let b = a;` moving a live root or an outer binding in
            // is judged by the escape/domain gates below the root
            // registration, not by the root-safety closure (whose
            // closed world would misjudge the source type).
            let value_is_move = matches!(value, Node::Identifier(_));
            if !value_is_move {
                self.arena_check_root_safe(name, ty)?;
            }
            let captured = self.expr_reaches_current_root(value);
            with_stack(|s| {
                if let Some(f) = s.last_mut() {
                    f.roots.push(name.to_string());
                    if captured {
                        // Route 4 taint is about VALUES that can carry
                        // the capture out (closures): a plain aggregate
                        // holding a root is itself arena-owned and is
                        // judged by the escape gates on its own root.
                        let closure_like = matches!(
                            value,
                            Node::ClosureLiteral(_) | Node::Identifier(_)
                        );
                        if closure_like {
                            f.tainted_closures.push(name.to_string());
                        }
                    }
                }
            });
            return Ok(());
        }
        Ok(())
    }

    /// Which live arena frame (index into the stack) owns `name` as a
    /// root, if any.
    fn arena_root_domain(&self, name: &str) -> Option<usize> {
        with_stack(|s| {
            s.iter()
                .rposition(|f| f.roots.iter().any(|r| r == name))
        })
    }

    fn is_current_root(&self, name: &str) -> bool {
        with_stack(|s| {
            s.last()
                .map(|f| f.roots.iter().any(|r| r == name))
                .unwrap_or(false)
        })
    }

    /// ArenaUnsafe gate: the root type's closure must stay inside the
    /// flat value world (design 2.2 / 2.4).
    fn arena_check_root_safe(&mut self, name: &str, ty: &Type) -> CheckResult<()> {
        let mut visiting: HashSet<String> = HashSet::new();
        if let Some((bad, class)) = self.type_closure_banned(ty, &mut visiting) {
            return Err(err(
                ErrorKind::ArenaUnsafe,
                format!(
                    "ArenaUnsafe: arena root `{name}` of type {} reaches forbidden type {bad} ({class}) in its type closure",
                    crate::types_util::type_desc(ty)
                ),
            ));
        }
        Ok(())
    }

    /// Greatest-fixpoint closure walk: returns the first banned leaf
    /// `(type description, class)` reachable from `ty`. `visiting`
    /// accumulates every user type already entered — recursion is
    /// NOT cut for reachability (a recursive type is unsafe iff any
    /// unfolding reaches a banned leaf), so a name is only skipped
    /// after its full expansion found nothing.
    fn type_closure_banned(
        &mut self,
        ty: &Type,
        visiting: &mut HashSet<String>,
    ) -> Option<(String, &'static str)> {
        match ty {
            Type::Pointer(_) => Some(("raw pointer".to_string(), "raw pointer")),
            Type::Borrow(_) => Some(("borrow".to_string(), "borrow view")),
            Type::Array(at) => self.type_closure_banned(&at.elem, visiting),
            Type::Tuple(tt) => {
                for e in &tt.elems {
                    if let Some(b) = self.type_closure_banned(e, visiting) {
                        return Some(b);
                    }
                }
                None
            }
            Type::Future(inner) => self.type_closure_banned(inner, visiting),
            Type::UserDefined(ud) => {
                let (name, generics) = (&ud.name, &ud.generics);
                let base = name.rsplit("::").next().unwrap_or(name.as_str());
                if BANNED.contains(&base) {
                    return Some((base.to_string(), "forbidden arena type class"));
                }
                if base == "Vec" || base == "GrowVec" {
                    if let Some(elem) = generics.first() {
                        return self.type_closure_banned(&elem.clone(), visiting);
                    }
                    return None;
                }
                if base == "Option" || base == "Result" {
                    for g in generics.clone() {
                        if let Some(b) = self.type_closure_banned(&g, visiting) {
                            return Some(b);
                        }
                    }
                    return None;
                }
                if SAFE_LEAVES.contains(&base) {
                    return None;
                }
                if visiting.contains(base) {
                    return None;
                }
                // User struct: expand fields with generic substitution.
                if let Some(info) = self.structs.get(base).cloned() {
                    if info.is_union {
                        return Some((base.to_string(), "union"));
                    }
                    if info.is_opaque {
                        return Some((base.to_string(), "opaque type"));
                    }
                    visiting.insert(base.to_string());
                    let params = info.generics.clone();
                    for (_, fty) in &info.fields {
                        let sub = substitute_generics(fty, &params, generics);
                        if let Some(b) = self.type_closure_banned(&sub, visiting) {
                            return Some(b);
                        }
                    }
                    return None;
                }
                if let Some(variants) = self.enum_variants.get(base).cloned() {
                    visiting.insert(base.to_string());
                    for v in &variants {
                        let params = v.generics.clone();
                        for f in &v.fields {
                            let sub = substitute_generics(&f.1, &params, generics);
                            if let Some(b) = self.type_closure_banned(&sub, visiting) {
                                return Some(b);
                            }
                        }
                    }
                    return None;
                }
                if self.aliases.contains_key(base) {
                    let target = self.aliases.get(base).cloned().unwrap();
                    visiting.insert(base.to_string());
                    return self.type_closure_banned(&target, visiting);
                }
                // Closed world: an undeclared type is unverifiable.
                Some((base.to_string(), "unverifiable (undeclared type)"))
            }
            _ => None,
        }
    }

    /// Does `expr` reach a root of the CURRENT (innermost) arena
    /// domain by value? Walks literals, moves, casts, field/index
    /// projections and closure bodies; call results are fresh values
    /// (only a call's by-value root ARGUMENT is an escape, handled by
    /// the call-arg gate, and a tainted closure callee is an escape).
    pub fn expr_reaches_current_root(&self, expr: &Node) -> bool {
        match expr {
            Node::Identifier(name) => {
                self.is_current_root(name)
                    || with_stack(|s| {
                        s.last()
                            .map(|f| f.tainted_closures.iter().any(|t| t == name))
                            .unwrap_or(false)
                    })
            }
            Node::MoveExpr(m) => self.expr_reaches_current_root(&m.expr),
            Node::CastExpr(c) => self.expr_reaches_current_root(&c.expr),
            Node::TryExpr(t) => self.expr_reaches_current_root(&t.expr),
            Node::FieldExpr(f) => self.expr_reaches_current_root(&f.expr),
            Node::IndexExpr(i) => self.expr_reaches_current_root(&i.target),
            Node::StructLiteral(lit) => lit
                .fields
                .iter()
                .any(|f| self.expr_reaches_current_root(&f.value)),
            Node::EnumLiteral(lit) => lit
                .fields
                .iter()
                .any(|f| self.expr_reaches_current_root(&f.value)),
            Node::TupleLiteral(lit) => lit
                .elements
                .iter()
                .any(|e| self.expr_reaches_current_root(e)),
            Node::ArrayLiteral(lit) => lit
                .elements
                .iter()
                .any(|e| self.expr_reaches_current_root(e)),
            Node::ClosureLiteral(c) => self.expr_mentions_current_root(&c.body),
            Node::CallExpr(call) => {
                // A call THROUGH a tainted closure binding invokes
                // the capture: `return f(1)` moves the closure (and
                // with it the root it captured) out of the domain.
                // The callee binding arrives either as `call.callee`
                // (immediately-invoked form) or, for bare-name calls,
                // as `call.func_name` with no callee/associated
                // target — the second shape is the ordinary one and
                // was previously unexamined (R-3 closure_capture).
                let callee_name = match &call.callee {
                    Some(c) => match c.as_ref() {
                        Node::Identifier(name) => Some(name.as_str()),
                        _ => None,
                    },
                    None => {
                        if call.associated_target.is_none() {
                            Some(call.func_name.as_str())
                        } else {
                            None
                        }
                    }
                };
                if let Some(name) = callee_name {
                    if with_stack(|s| {
                        s.last()
                            .map(|f| f.tainted_closures.iter().any(|t| t == name))
                            .unwrap_or(false)
                    }) {
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }

    /// Deep mention scan for closure bodies ONLY (route 4 taint):
    /// a closure whose body mentions a current root anywhere —
    /// including inside call arguments (`len(v)`) — captures it.
    /// Ordinary reachability deliberately does NOT scan call
    /// arguments (a call RESULT is a fresh value: clone_out).
    fn expr_mentions_current_root(&self, expr: &Node) -> bool {
        if self.expr_reaches_current_root(expr) {
            return true;
        }
        match expr {
            Node::CallExpr(call) => {
                call.args.iter().any(|a| self.expr_mentions_current_root(a))
                    || call
                        .callee
                        .as_deref()
                        .map(|c| self.expr_mentions_current_root(c))
                        .unwrap_or(false)
            }
            Node::BinaryExpr(b) => {
                self.expr_mentions_current_root(&b.left)
                    || self.expr_mentions_current_root(&b.right)
            }
            Node::CastExpr(c) => self.expr_mentions_current_root(&c.expr),
            Node::MoveExpr(m) => self.expr_mentions_current_root(&m.expr),
            Node::BorrowExpr(b) => self.expr_mentions_current_root(&b.expr),
            Node::TryExpr(t) => self.expr_mentions_current_root(&t.expr),
            Node::FieldExpr(f) => self.expr_mentions_current_root(&f.expr),
            Node::IndexExpr(i) => {
                self.expr_mentions_current_root(&i.target)
                    || self.expr_mentions_current_root(&i.index)
            }
            Node::StructLiteral(lit) => lit
                .fields
                .iter()
                .any(|f| self.expr_mentions_current_root(&f.value)),
            Node::EnumLiteral(lit) => lit
                .fields
                .iter()
                .any(|f| self.expr_mentions_current_root(&f.value)),
            Node::TupleLiteral(lit) => lit
                .elements
                .iter()
                .any(|e| self.expr_mentions_current_root(e)),
            Node::ArrayLiteral(lit) => lit
                .elements
                .iter()
                .any(|e| self.expr_mentions_current_root(e)),
            _ => false,
        }
    }

    /// Return gate (escape route 1): a return value that reaches a
    /// current-domain root moves block memory to the caller.
    pub fn arena_check_return(&self, value: Option<&Node>) -> CheckResult<()> {
        if !arena_active() {
            return Ok(());
        }
        if let Some(v) = value {
            if self.expr_reaches_current_root(v) {
                return Err(err(
                    ErrorKind::ArenaEscape,
                    "ArenaEscape: returning an arena-owned value moves it out of its arena domain; clone it out explicitly".to_string(),
                ));
            }
        }
        Ok(())
    }

    /// Store gate (escape route 2): assigning a current-domain root
    /// into a place owned by an outer domain.
    pub fn arena_check_assign(
        &self,
        target: &Node,
        value: &Node,
    ) -> CheckResult<()> {
        if !arena_active() {
            return Ok(());
        }
        if let Node::Identifier(name) = target {
            if !self.is_current_root(name) && self.expr_reaches_current_root(value) {
                return Err(err(
                    ErrorKind::ArenaEscape,
                    format!(
                        "ArenaEscape: storing an arena-owned value into outer-domain place `{name}` moves it out of its arena domain"
                    ),
                ));
            }
        }
        Ok(())
    }

    /// Call gates (escape routes 3, 5, 6): by-value root arguments.
    /// `recv_ty` is the receiver type for method-form calls (args[0]
    /// is the receiver); `None` for plain calls.
    pub fn arena_check_call(
        &self,
        call: &sla_ast::exprs::CallExpr,
        recv_ty: Option<&Type>,
    ) -> CheckResult<()> {
        if !arena_active() {
            return Ok(());
        }
        let is_method = recv_ty.is_some();
        // Route 6: `go` spawn of a root (parser wraps the spawn in a
        // call named `go` — see the go slice; until that slice lands
        // this arm is inert).
        if call.func_name == "go" {
            for a in &call.args {
                if self.expr_reaches_current_root(a) {
                    return Err(err(
                        ErrorKind::ArenaGoroutine,
                        "ArenaGoroutine: passing an arena-owned value to a spawned function crosses the goroutine boundary; clone it out first".to_string(),
                    ));
                }
            }
            return Ok(());
        }
        // Route 5: channel send of a root.
        if is_method && call.func_name == "send" {
            if let Some(rt) = recv_ty {
                if let Type::UserDefined(ud) = rt {
                    let base = ud.name.rsplit("::").next().unwrap_or(ud.name.as_str());
                    if base == "Chan" || base == "Sender" {
                        for a in call.args.iter().skip(1) {
                            if self.expr_reaches_current_root(a) {
                                return Err(err(
                                    ErrorKind::ArenaGoroutine,
                                    "ArenaGoroutine: sending an arena-owned value on a channel crosses the goroutine boundary; clone it out first".to_string(),
                                ));
                            }
                        }
                        return Ok(());
                    }
                }
            }
        }
        // Route 3: by-value root argument to an ordinary callee.
        // Exemptions: borrow arguments (not Identifier nodes),
        // method calls whose receiver is itself a current root
        // (same-domain container traffic: push/insert), and macro
        // calls (their arguments are token material, expanded in
        // place — e.g. `assert_eq!(root, ..)` reads). Plain-call
        // arguments are gated POST-resolution (see
        // `arena_check_plain_call_args`), because the R-6/D-C7
        // capability and consumption discipline must judge first;
        // this pre-pass only judges method-form calls, whose args
        // have not been consumed yet at this point.
        if !is_method {
            return Ok(());
        }
        if self.macros.contains_key(&call.func_name) {
            return Ok(());
        }
        if is_method {
            if let Some(Node::Identifier(recv)) = call.args.first() {
                if self.is_current_root(recv) {
                    return Ok(());
                }
            }
        }
        let start = if is_method { 1 } else { 0 };
        for a in call.args.iter().skip(start) {
            if let Node::Identifier(name) = a {
                if self.is_current_root(name) {
                    return Err(err(
                        ErrorKind::ArenaEscape,
                        format!(
                            "ArenaEscape: passing arena-owned value `{name}` by value to `{}` moves it out of its arena domain; pass a borrow or clone it",
                            call.func_name
                        ),
                    ));
                }
            }
        }
        Ok(())
    }
}

/// Substitute struct/enum generic parameters by the use-site
/// arguments (design 2.2: generic substitution is a closure edge).
pub fn substitute_generics(ty: &Type, params: &[String], args: &[Type]) -> Type {
    match ty {
        Type::UserDefined(ud) if ud.generics.is_empty() => {
            if let Some(pos) = params.iter().position(|p| p == &ud.name) {
                if let Some(arg) = args.get(pos) {
                    return arg.clone();
                }
            }
            ty.clone()
        }
        Type::UserDefined(ud) => Type::UserDefined(sla_ast::types::UserDefined {
            name: ud.name.clone(),
            generics: ud
                .generics
                .iter()
                .map(|g| substitute_generics(g, params, args))
                .collect(),
        }),
        Type::Array(at) => Type::Array(sla_ast::types::ArrayType {
            elem: Box::new(substitute_generics(&at.elem, params, args)),
            len: at.len,
        }),
        Type::Tuple(tt) => Type::Tuple(sla_ast::types::TupleType {
            elems: tt
                .elems
                .iter()
                .map(|e| substitute_generics(e, params, args))
                .collect(),
        }),
        Type::Borrow(inner) => {
            Type::Borrow(Box::new(substitute_generics(inner, params, args)))
        }
        Type::Pointer(inner) => {
            Type::Pointer(Box::new(substitute_generics(inner, params, args)))
        }
        _ => ty.clone(),
    }
}

impl Checker {
    /// Route 3 for PLAIN calls, run after the call has been fully
    /// checked (resolution + argument discipline + consumption):
    /// a current-root binding that the call CONSUMED is an escape.
    /// Borrow reads and preserved copies stay Active and pass.
    pub fn arena_check_plain_call_args(&mut self, call: &CallExpr) {
        if !arena_active() || call.associated_target.is_some() {
            return;
        }
        if self.macros.contains_key(&call.func_name) {
            return;
        }
        // Signature-based route 3 (design 3): ANY by-value pass of
        // a current root to a plain user function moves it out of
        // the domain — independent of the checker's consumption
        // ledger (D-C7/C1: ordinary call args are not consumed;
        // Vec/GrowVec args are emitter-cloned). Borrow parameters,
        // borrow/pointer-typed parameters, and Copy arguments are
        // reads, not moves. Overload sets are judged only when the
        // matching candidate is unambiguous by arity.
        let sig_params: Option<Vec<(bool, bool, Type)>> = if let Some(sig) =
            self.funcs.get(&call.func_name).or_else(|| {
                self.imported_funcs.get(&call.func_name)
            }) {
            Some(
                sig.params
                    .iter()
                    .map(|p| (p.is_borrow, p.is_move, p.ty.clone()))
                    .collect(),
            )
        } else if let Some(set) = self.func_overload_sets.get(&call.func_name) {
            let mut by_arity = set
                .iter()
                .filter(|s| s.params.len() == call.args.len());
            match (by_arity.next(), by_arity.next()) {
                (Some(only), None) => Some(
                    only.params
                        .iter()
                        .map(|p| (p.is_borrow, p.is_move, p.ty.clone()))
                        .collect(),
                ),
                _ => None,
            }
        } else {
            None
        };
        let Some(params) = sig_params else {
            return;
        };
        for (idx, a) in call.args.iter().enumerate() {
            if let Node::Identifier(name) = a {
                if !self.is_current_root(name) {
                    continue;
                }
                let Some((is_borrow, _is_move, pty)) = params.get(idx) else {
                    continue;
                };
                if *is_borrow {
                    continue;
                }
                if matches!(pty, Type::Borrow(_) | Type::Pointer(_)) {
                    continue;
                }
                if let Some(t) = self.lookup_ty(name) {
                    if self.type_is_copy(&t) {
                        continue;
                    }
                } else {
                    continue;
                }
                // Recorded for the caller to raise (the call's own
                // result type is already computed).
                self.arena_pending_escape = Some(format!(
                    "ArenaEscape: passing arena-owned value `{name}` by value to `{}` moves it out of its arena domain; pass a borrow or clone it",
                    call.func_name
                ));
                return;
            }
        }
    }

}
