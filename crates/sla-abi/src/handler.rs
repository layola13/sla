//! Second ABI: the state-handler compiler entry points
//! (`plugin_handler.zig`), used by the host when compiling SAX/host
//! state handlers. All three structs are `extern struct` in the
//! original; sizes 40 / 16 / 48 B per docs/ABI_MAP.md §5.
//!
//! Ownership contract (original): `body`/`support` in the result are
//! allocated with the C allocator inside the plugin, ownership moves
//! to the host, and the host must call `sla_compile_handler_result_free`
//! to hand them back. `error_name` points at a static string and is
//! never freed. Return codes: 0 success, 1 invalid argument
//! (`error_name` says which), 2 compile failure (`error_name` is the
//! error name).
//!
//! This crate carries the types only; the exported functions
//! themselves are wired in `sla-plugin` once the handler compiler is
//! ported.

/// State-field type codes for `SlaHandlerStateFieldAbi::ty`.
pub const HANDLER_TY_I1: u32 = 1;
/// See [`HANDLER_TY_I1`].
pub const HANDLER_TY_I32: u32 = 2;
/// See [`HANDLER_TY_I1`].
pub const HANDLER_TY_I64: u32 = 3;
/// See [`HANDLER_TY_I1`].
pub const HANDLER_TY_F64: u32 = 4;
/// See [`HANDLER_TY_I1`]. Unknown codes are rejected by the original
/// with `InvalidStateFieldType`.
pub const HANDLER_TY_PTR: u32 = 5;

/// One state field of a handler (40 B).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SlaHandlerStateFieldAbi {
    pub name_ptr: *const u8,
    pub name_len: usize,
    pub ty: u32,
    pub address_ptr: *const u8,
    pub address_len: usize,
}

/// Compile options (16 B); the pointer itself may be null at the call.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SlaCompileHandlerOptionsAbi {
    pub base_dir_ptr: *const u8,
    pub base_dir_len: usize,
}

/// Compile result (48 B). See the module docs for ownership.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SlaCompileHandlerResultAbi {
    pub body_ptr: *const u8,
    pub body_len: usize,
    pub support_ptr: *const u8,
    pub support_len: usize,
    pub error_name_ptr: *const u8,
    pub error_name_len: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    #[test]
    fn handler_layouts_match_abi_map() {
        assert_eq!(size_of::<SlaHandlerStateFieldAbi>(), 40);
        assert_eq!(offset_of!(SlaHandlerStateFieldAbi, name_ptr), 0);
        assert_eq!(offset_of!(SlaHandlerStateFieldAbi, name_len), 8);
        assert_eq!(offset_of!(SlaHandlerStateFieldAbi, ty), 16);
        assert_eq!(offset_of!(SlaHandlerStateFieldAbi, address_ptr), 24);
        assert_eq!(offset_of!(SlaHandlerStateFieldAbi, address_len), 32);
        assert_eq!(size_of::<SlaCompileHandlerOptionsAbi>(), 16);
        assert_eq!(size_of::<SlaCompileHandlerResultAbi>(), 48);
        assert_eq!(offset_of!(SlaCompileHandlerResultAbi, body_ptr), 0);
        assert_eq!(offset_of!(SlaCompileHandlerResultAbi, support_ptr), 16);
        assert_eq!(offset_of!(SlaCompileHandlerResultAbi, error_name_ptr), 32);
    }
}
