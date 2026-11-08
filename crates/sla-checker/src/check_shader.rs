//! R-3 (J-1): the shader sublanguage checker (DESIGN.md 6.2
//! subset, judged by the corpus in corpus/tests).
//!
//! Registration: `@sublanguage shader;` is the only bound
//! kind; any other name is SublanguageNotBound. Each shader
//! is then validated:
//! - one name may be declared only once across ALL of the
//!   shader's qualifiers (@global structs and their fields
//!   live in their own scopes) — ShaderDuplicateName;
//! - stage bodies are lexed and expression-checked with a
//!   small typed evaluator over the shader value types
//!   (vec2/3/4, mat4, sampler2D, scalars, bool): vector
//!   arithmetic requires equal dimensions —
//!   ShaderDimensionMismatch; integer literals beyond the
//!   i64 range are Overflow (the sub-parser's parseInt);
//! - @input values may only be READ in the vertex stage —
//!   ShaderStageViolation.
//!
//! The evaluator is deliberately small: identifiers,
//! literals, field access / swizzles, calls (vecN
//! constructors, texture `.get`), unary/binary operators,
//! assignments and `if` blocks. Anything outside that shape
//! is tolerated (no diagnostic) rather than guessed at.

use std::collections::HashMap;
use std::collections::HashSet;

use sla_ast::node::Node;
use sla_ast::Program;

use crate::diag::{CheckResult, Diagnostic, ErrorKind};

/// The sublanguage kinds this plugin has a sub-parser for.
pub const BOUND_SUBLANGUAGES: &[&str] = &["shader"];

#[derive(Debug, Clone, PartialEq)]
enum SType {
    Scalar,
    Bool,
    Vec(u8),
    Mat4,
    Sampler,
    Unknown,
}

fn stype_of(name: &str) -> SType {
    match name {
        "vec2" => SType::Vec(2),
        "vec3" => SType::Vec(3),
        "vec4" => SType::Vec(4),
        "mat4" => SType::Mat4,
        "sampler2D" => SType::Sampler,
        "bool" => SType::Bool,
        _ => SType::Scalar,
    }
}

fn dim_mismatch(what: &str) -> Diagnostic {
    Diagnostic::new(
        ErrorKind::ShaderDimensionMismatch,
        format!("ShaderDimensionMismatch: {what}"),
    )
}

impl crate::checker::Checker {
    /// Validate sublanguage registrations and shader bodies.
    /// Called from check_program alongside the hook pass.
    pub fn apply_sublanguage_checks(
        &mut self,
        program: &Program,
    ) -> CheckResult<()> {
        for decl in &program.decls {
            if let Node::SublanguageDecl(s) = decl {
                if !BOUND_SUBLANGUAGES.contains(&s.name.as_str()) {
                    return Err(Diagnostic::new(
                        ErrorKind::SublanguageNotBound,
                        format!(
                            "SublanguageNotBound: no sub-parser is bound for sublanguage `{}`",
                            s.name
                        ),
                    ));
                }
            }
        }
        for decl in &program.decls {
            if let Node::ShaderDecl(shader) = decl {
                check_shader(shader)?;
            }
        }
        Ok(())
    }
}

fn check_shader(shader: &sla_ast::ShaderDecl) -> CheckResult<()> {
    // Duplicate rule: one name once across the shader's
    // qualifier namespaces (globals' own names included;
    // their FIELDS are scoped to the struct).
    let mut seen: HashSet<String> = HashSet::new();
    let mut claim = |name: &str| -> CheckResult<()> {
        if !seen.insert(name.to_string()) {
            return Err(Diagnostic::new(
                ErrorKind::ShaderDuplicateName,
                format!(
                    "ShaderDuplicateName: `{name}` is declared more than once in shader `{}`",
                    shader.name
                ),
            ));
        }
        Ok(())
    };
    for g in &shader.globals {
        claim(&g.name)?;
    }
    for f in &shader.inputs {
        claim(&f.name)?;
    }
    for f in &shader.params {
        claim(&f.name)?;
    }
    for c in &shader.consts {
        claim(&c.name)?;
    }
    for f in &shader.vars {
        claim(&f.name)?;
    }

    // Value environment for stage bodies.
    let mut env: HashMap<String, SType> = HashMap::new();
    for f in &shader.inputs {
        env.insert(f.name.clone(), stype_of(&f.ty));
    }
    for f in &shader.params {
        env.insert(f.name.clone(), stype_of(&f.ty));
    }
    for c in &shader.consts {
        env.insert(c.name.clone(), stype_of(&c.ty));
    }
    for f in &shader.vars {
        env.insert(f.name.clone(), stype_of(&f.ty));
    }
    // @global structs are accessed as `Struct.field`.
    let mut global_fields: HashMap<String, HashMap<String, SType>> =
        HashMap::new();
    for g in &shader.globals {
        let mut fields = HashMap::new();
        for f in &g.fields {
            fields.insert(f.name.clone(), stype_of(&f.ty));
        }
        global_fields.insert(g.name.clone(), fields);
    }
    env.insert("clip_position".into(), SType::Vec(4));
    env.insert("frag_color".into(), SType::Vec(4));

    let input_names: HashSet<&str> =
        shader.inputs.iter().map(|f| f.name.as_str()).collect();
    for (stage, raw) in &shader.stages {
        let mut cx = BodyCx {
            env: &env,
            global_fields: &global_fields,
            input_names: &input_names,
            stage: stage.as_str(),
        };
        cx.check_body(raw)?;
    }
    Ok(())
}

// -- stage body evaluator -------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Ident(String),
    Int(String),
    Float,
    Punct(char),
}

fn lex_body(raw: &str) -> Vec<Tok> {
    let bytes = raw.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c.is_whitespace() {
            i += 1;
        } else if c == '/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            while i < bytes.len()
                && ((bytes[i] as char).is_ascii_alphanumeric() || bytes[i] == b'_')
            {
                i += 1;
            }
            out.push(Tok::Ident(raw[start..i].to_string()));
        } else if c.is_ascii_digit() {
            let start = i;
            let mut is_float = false;
            while i < bytes.len()
                && ((bytes[i] as char).is_ascii_digit() || bytes[i] == b'.')
            {
                if bytes[i] == b'.' {
                    // `1.0` is a float; `1.x` would be member
                    // access on an int — only float when a digit
                    // follows the dot.
                    if i + 1 < bytes.len() && (bytes[i + 1] as char).is_ascii_digit()
                    {
                        is_float = true;
                    } else {
                        break;
                    }
                }
                i += 1;
            }
            if is_float {
                out.push(Tok::Float);
            } else {
                out.push(Tok::Int(raw[start..i].to_string()));
            }
        } else {
            out.push(Tok::Punct(c));
            i += 1;
        }
    }
    out
}

struct BodyCx<'a> {
    env: &'a HashMap<String, SType>,
    global_fields: &'a HashMap<String, HashMap<String, SType>>,
    input_names: &'a HashSet<&'a str>,
    stage: &'a str,
}

impl<'a> BodyCx<'a> {
    fn check_body(&mut self, raw: &str) -> CheckResult<()> {
        let toks = lex_body(raw);
        let mut pos = 0usize;
        self.check_stmts(&toks, &mut pos, toks.len())
    }

    fn check_stmts(
        &mut self,
        toks: &[Tok],
        pos: &mut usize,
        end: usize,
    ) -> CheckResult<()> {
        while *pos < end {
            match &toks[*pos] {
                Tok::Punct(';') => *pos += 1,
                Tok::Ident(w) if w == "if" => {
                    *pos += 1;
                    // condition expression up to `{`
                    let _ = self.eval_expr(toks, pos, end)?;
                    if *pos < end && toks[*pos] == Tok::Punct('{') {
                        let close = matching_brace(toks, *pos, end);
                        let mut inner = *pos + 1;
                        self.check_stmts(toks, &mut inner, close)?;
                        *pos = close + 1;
                    }
                }
                Tok::Ident(_) => {
                    // assignment or expression statement
                    let _ = self.eval_expr(toks, pos, end)?;
                    if *pos < end && toks[*pos] == Tok::Punct('=') {
                        *pos += 1;
                        let _ = self.eval_expr(toks, pos, end)?;
                    }
                    if *pos < end && toks[*pos] == Tok::Punct(';') {
                        *pos += 1;
                    }
                }
                _ => *pos += 1,
            }
        }
        Ok(())
    }

    /// Evaluate an expression's TYPE, consuming tokens. Stops
    /// at `;`, `=`, `)` `,` or the end. Returns Unknown for
    /// shapes outside the subset (tolerated).
    fn eval_expr(
        &mut self,
        toks: &[Tok],
        pos: &mut usize,
        end: usize,
    ) -> CheckResult<SType> {
        let mut lhs = self.eval_unary(toks, pos, end)?;
        while *pos < end {
            let op = match &toks[*pos] {
                Tok::Punct(c) if *c == '+' || *c == '-' || *c == '*' || *c == '/' => {
                    *c
                }
                _ => break,
            };
            *pos += 1;
            let rhs = self.eval_unary(toks, pos, end)?;
            lhs = combine(op, &lhs, &rhs)?;
        }
        Ok(lhs)
    }

    fn eval_unary(
        &mut self,
        toks: &[Tok],
        pos: &mut usize,
        end: usize,
    ) -> CheckResult<SType> {
        if *pos >= end {
            return Ok(SType::Unknown);
        }
        match &toks[*pos] {
            Tok::Punct('-') => {
                *pos += 1;
                self.eval_unary(toks, pos, end)
            }
            Tok::Punct('(') => {
                *pos += 1;
                let t = self.eval_expr(toks, pos, end)?;
                if *pos < end && toks[*pos] == Tok::Punct(')') {
                    *pos += 1;
                }
                Ok(t)
            }
            Tok::Int(text) => {
                let text = text.clone();
                *pos += 1;
                if text.parse::<i64>().is_err() && text.parse::<u64>().is_err() {
                    // Digits only (lexer guarantee): beyond u64
                    // is beyond i64 — the sub-parser's parseInt
                    // Overflow.
                    return Err(Diagnostic::new(
                        ErrorKind::Overflow,
                        format!("Overflow: integer literal `{text}` exceeds the shader integer range"),
                    ));
                }
                Ok(SType::Scalar)
            }
            Tok::Float => {
                *pos += 1;
                Ok(SType::Scalar)
            }
            Tok::Ident(name) => {
                let name = name.clone();
                *pos += 1;
                self.eval_ident_tail(name, toks, pos, end)
            }
            _ => {
                *pos += 1;
                Ok(SType::Unknown)
            }
        }
    }

    fn eval_ident_tail(
        &mut self,
        name: String,
        toks: &[Tok],
        pos: &mut usize,
        end: usize,
    ) -> CheckResult<SType> {
        // Stage rule: reading an @input outside vertex.
        if self.input_names.contains(name.as_str()) && self.stage != "vertex" {
            return Err(Diagnostic::new(
                ErrorKind::ShaderStageViolation,
                format!(
                    "ShaderStageViolation: @input `{name}` may only be read in the vertex stage, not {}",
                    self.stage
                ),
            ));
        }
        // vecN(...) constructor.
        if matches!(name.as_str(), "vec2" | "vec3" | "vec4")
            && *pos < end
            && toks[*pos] == Tok::Punct('(')
        {
            let n: u8 = name[3..].parse().unwrap_or(0);
            *pos += 1;
            // Argument dimensions must sum to n (a single
            // scalar splat is also legal).
            let mut total: i32 = 0;
            let mut any_scalar = false;
            let mut argc = 0usize;
            while *pos < end && toks[*pos] != Tok::Punct(')') {
                let at = self.eval_expr(toks, pos, end)?;
                match at {
                    SType::Vec(d) => total += d as i32,
                    SType::Scalar => {
                        total += 1;
                        any_scalar = true;
                    }
                    _ => {}
                }
                argc += 1;
                if *pos < end && toks[*pos] == Tok::Punct(',') {
                    *pos += 1;
                }
            }
            if *pos < end {
                *pos += 1; // ')'
            }
            if argc > 1 && total != n as i32 {
                return Err(dim_mismatch(&format!(
                    "vec{n} constructor arguments sum to {total} components"
                )));
            }
            let _ = any_scalar;
            return Ok(SType::Vec(n));
        }
        let mut ty = self.env.get(&name).cloned().unwrap_or(SType::Unknown);
        // Member tails: `.field`, `.get(...)`, swizzles.
        while *pos + 1 < end && toks[*pos] == Tok::Punct('.') {
            let member = match &toks[*pos + 1] {
                Tok::Ident(m) => m.clone(),
                _ => break,
            };
            *pos += 2;
            if member == "get"
                && *pos < end
                && toks[*pos] == Tok::Punct('(')
            {
                *pos += 1;
                let _ = self.eval_expr(toks, pos, end)?;
                if *pos < end && toks[*pos] == Tok::Punct(')') {
                    *pos += 1;
                }
                ty = SType::Vec(4);
                continue;
            }
            if let Some(fields) = self.global_fields.get(&name) {
                ty = fields.get(&member).cloned().unwrap_or(SType::Unknown);
                continue;
            }
            // Swizzle on a vector: dimension = member length
            // (xyzw/rgba sets), single char = scalar.
            if let SType::Vec(_) = ty {
                let len = member.chars().count();
                if (1..=4).contains(&len)
                    && member
                        .chars()
                        .all(|c| "xyzwrgbastpq".contains(c))
                {
                    ty = if len == 1 {
                        SType::Scalar
                    } else {
                        SType::Vec(len as u8)
                    };
                    continue;
                }
            }
            ty = SType::Unknown;
        }
        Ok(ty)
    }
}

fn matching_brace(toks: &[Tok], open: usize, end: usize) -> usize {
    let mut depth = 0usize;
    let mut i = open;
    while i < end {
        match &toks[i] {
            Tok::Punct('{') => depth += 1,
            Tok::Punct('}') => {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
            }
            _ => {}
        }
        i += 1;
    }
    end
}

fn combine(op: char, lhs: &SType, rhs: &SType) -> CheckResult<SType> {
    match (lhs, rhs) {
        (SType::Unknown, _) | (_, SType::Unknown) => Ok(SType::Unknown),
        (SType::Vec(a), SType::Vec(b)) => {
            if a == b {
                Ok(SType::Vec(*a))
            } else {
                Err(dim_mismatch(&format!(
                    "cannot apply `{op}` to vec{a} and vec{b}"
                )))
            }
        }
        (SType::Vec(n), SType::Scalar) | (SType::Scalar, SType::Vec(n)) => {
            Ok(SType::Vec(*n))
        }
        (SType::Mat4, SType::Vec(4)) => Ok(SType::Vec(4)),
        (SType::Vec(4), SType::Mat4) => Ok(SType::Vec(4)),
        (SType::Mat4, SType::Mat4) => Ok(SType::Mat4),
        (SType::Mat4, SType::Scalar) | (SType::Scalar, SType::Mat4) => {
            Ok(SType::Mat4)
        }
        (SType::Scalar, SType::Scalar) => Ok(SType::Scalar),
        (SType::Bool, SType::Bool) => Ok(SType::Bool),
        _ => Ok(SType::Unknown),
    }
}
