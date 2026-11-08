//! Template-literal parsing (Zig `parseTemplateLiteral` +
//! `appendTemplateChunk` in parser.zig), split out of prefix.rs
//! for the 700-line file cap.
//!
//! Desugar: `` `head ${a} tail` `` -> `format("head {} tail", a)`.
//! Interpolation reuses the existing `format()` machinery, so the
//! supported value types are exactly `format()`'s (strings, ints,
//! floats, bools). A template without interpolation degrades to a
//! plain string literal. Literal chunks are re-escaped for the
//! format string: `\${` stays `${`, a backslash-escaped backtick
//! becomes a plain backtick, `"`/`{`/`}` are doubled/escaped, and
//! any other backslash pair passes through verbatim.

use sla_ast::exprs::{CallExpr, Literal};
use sla_ast::node::Node;
use sla_lexer::Tag;

use crate::error::{ParseError, ParseErrorKind};
use crate::parser::Parser;

/// Zig `appendTemplateChunk`: append one raw literal chunk to the
/// format-string under construction, applying the escapes above.
fn append_template_chunk(out: &mut String, raw: &str) {
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'\\' {
            if i + 1 >= bytes.len() {
                out.push(c as char);
                break;
            }
            let n = bytes[i + 1];
            if n == b'$' && i + 2 < bytes.len() && bytes[i + 2] == b'{' {
                out.push_str("${");
                i += 3;
                continue;
            } else if n == b'`' {
                out.push('`');
                i += 2;
                continue;
            } else {
                out.push(c as char);
                out.push(n as char);
                i += 2;
                continue;
            }
        }
        match c {
            b'"' => out.push_str("\\\""),
            b'{' => out.push_str("{{"),
            b'}' => out.push_str("}}"),
            _ => {
                // Byte-wise copy would split multi-byte UTF-8; the
                // chunk is valid source text, so copy the whole
                // char. ASCII fast path is the branch above for the
                // escaped specials; everything else lands here.
                let ch_len = utf8_len(bytes[i]);
                out.push_str(&raw[i..i + ch_len]);
            }
        }
        i += 1;
    }
}

fn utf8_len(first: u8) -> usize {
    if first < 0x80 {
        1
    } else if first >> 5 == 0b110 {
        2
    } else if first >> 4 == 0b1110 {
        3
    } else {
        4
    }
}

impl<'a> Parser<'a> {
    /// Zig `parseTemplateLiteral` (called on `template_start`).
    pub fn parse_template_literal(&mut self) -> Result<Node, ParseError> {
        self.expect(Tag::TemplateStart)?;
        let mut fmt_parts = String::new();
        let mut args: Vec<Node> = Vec::new();
        loop {
            match self.peek() {
                Tag::TemplateString => {
                    let raw = self.cur_text().to_string();
                    self.advance();
                    append_template_chunk(&mut fmt_parts, &raw);
                }
                Tag::TemplateInterp => {
                    self.advance();
                    let arg = self.parse_expr(0)?;
                    self.expect(Tag::RBrace)?;
                    fmt_parts.push_str("{}");
                    args.push(arg);
                }
                Tag::TemplateEnd => {
                    self.advance();
                    break;
                }
                _ => return Err(self.err_here(ParseErrorKind::SyntaxError)),
            }
        }
        let fmt_node = Node::Literal(Literal::StringVal(fmt_parts));
        if args.is_empty() {
            return Ok(fmt_node);
        }
        let mut call_args = Vec::with_capacity(args.len() + 1);
        call_args.push(fmt_node);
        call_args.extend(args);
        Ok(Node::CallExpr(CallExpr {
            func_name: "format".to_string(),
            associated_target: None,
            generics: Vec::new(),
            args: call_args,
            callee: None,
        }))
    }
}

#[cfg(test)]
mod tests_r3_template {
    use crate::parser::{Parser, ParserOptions};
    use sla_ast::exprs::Literal;
    use sla_ast::node::Node;

    fn parse_tpl(src: &str) -> Node {
        let mut p = Parser::with_options(src, ParserOptions::default());
        let root = p.parse_program().expect("template source must parse");
        let Node::Program(prog) = root else { panic!("shape") };
        let Node::FuncDecl(f) = &prog.decls[0] else { panic!("decl") };
        let Node::LetStmt(l) = &f.body[0] else { panic!("stmt") };
        (*l.value).clone()
    }

    #[test]
    fn template_with_interps_desugars_to_format_call() {
        // Corpus twin: demos/basic/94_nested_tpl (double interp).
        let v = parse_tpl("fn main() -> i32 { let s = `sum=${x + y} prod=${x * y}`; return 0; }");
        let Node::CallExpr(c) = v else { panic!("expected format call") };
        assert_eq!(c.func_name, "format");
        assert_eq!(c.args.len(), 3);
        let Node::Literal(Literal::StringVal(f)) = &c.args[0] else { panic!("fmt") };
        assert_eq!(f, "sum={} prod={}");
    }

    #[test]
    fn template_without_interp_is_plain_string() {
        let v = parse_tpl("fn main() -> i32 { let s = `plain`; return 0; }");
        let Node::Literal(Literal::StringVal(f)) = v else { panic!("expected literal") };
        assert_eq!(f, "plain");
    }

    #[test]
    fn template_chunk_braces_are_format_escaped() {
        let v = parse_tpl("fn main() -> i32 { let s = `a{b}c${x}`; return 0; }");
        let Node::CallExpr(c) = v else { panic!("expected format call") };
        let Node::Literal(Literal::StringVal(f)) = &c.args[0] else { panic!("fmt") };
        assert_eq!(f, "a{{b}}c{}");
    }
}
