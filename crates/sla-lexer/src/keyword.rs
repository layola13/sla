//! Keyword table, mirroring `checkKeyword` in the Zig lexer.
//! Anything not listed here lexes as `Tag::Identifier`.

use crate::token::Tag;

pub fn check_keyword(text: &[u8]) -> Tag {
    match text {
        b"struct" => Tag::KeywordStruct,
        b"union" => Tag::KeywordUnion,
        b"enum" => Tag::KeywordEnum,
        b"trait" => Tag::KeywordTrait,
        b"overload" => Tag::KeywordOverload,
        b"dyn" => Tag::KeywordDyn,
        b"impl" => Tag::KeywordImpl,
        b"mod" => Tag::KeywordMod,
        b"using" => Tag::KeywordUsing,
        b"pub" => Tag::KeywordPub,
        b"extern" => Tag::KeywordExtern,
        b"async" => Tag::KeywordAsync,
        b"await" => Tag::KeywordAwait,
        b"unsafe" => Tag::KeywordUnsafe,
        b"as" => Tag::KeywordAs,
        b"fn" => Tag::KeywordFn,
        b"if" => Tag::KeywordIf,
        b"else" => Tag::KeywordElse,
        b"match" => Tag::KeywordMatch,
        b"switch" => Tag::KeywordSwitch,
        b"return" => Tag::KeywordReturn,
        b"for" => Tag::KeywordFor,
        b"while" => Tag::KeywordWhile,
        b"break" => Tag::KeywordBreak,
        b"continue" => Tag::KeywordContinue,
        b"in" => Tag::KeywordIn,
        b"let" => Tag::KeywordLet,
        b"const" => Tag::KeywordConst,
        b"var" => Tag::KeywordVar,
        b"inline" => Tag::KeywordInline,
        b"macro" => Tag::KeywordMacro,
        b"mut" => Tag::KeywordMut,
        b"type" => Tag::KeywordType,
        _ => Tag::Identifier,
    }
}
