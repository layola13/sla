//! The host-built `Context` handed to every descriptor callback.
//!
//! In the original this is a PLAIN Zig struct on both sides (plugin
//! `plugin_api.Context`, host `sci/src/plugins.zig`) whose layout
//! agreement rests on "same source shape + same compiler" — divergence
//! D-C1. Measured on the reference implementation
//! (docs/probes/abi_probe.out.txt) the size is 72 B and Zig's auto
//! layout has REORDERED the fields by alignment: the declaration order
//! is allocator / host_version / log / log_ctx / json_mode /
//! broker_abi_version / broker_call / broker_ctx, but the physical
//! order is the one below. This mirror follows the MEASURED order; the
//! offsets are pinned by tests so a future "tidy-up" back to
//! declaration order fails loudly instead of silently misaligning
//! against the host.

use crate::descriptor::StrSlice;
use crate::LogLevel;
use std::os::raw::c_void;

/// The host allocator as an opaque (pointer, vtable) pair
/// (`std.mem.Allocator`, 16 B). The vtable's function-pointer
/// signatures are modeled for layout only — the plugin never calls
/// through them in this port (the original's command implementation
/// substitutes its own page/arena allocators anyway).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Allocator {
    pub ptr: *mut c_void,
    pub vtable: *const AllocatorVTable,
}

/// Layout model of `std.mem.Allocator.VTable` (four function pointers).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AllocatorVTable {
    pub alloc: extern "C" fn(
        ctx: *mut c_void,
        len: usize,
        alignment_log2: u8,
        ret_addr: usize,
    ) -> Option<*mut u8>,
    pub resize: extern "C" fn(
        ctx: *mut c_void,
        memory: *mut u8,
        memory_len: usize,
        alignment_log2: u8,
        new_len: usize,
        ret_addr: usize,
    ) -> bool,
    pub remap: extern "C" fn(
        ctx: *mut c_void,
        memory: *mut u8,
        memory_len: usize,
        alignment_log2: u8,
        new_len: usize,
        ret_addr: usize,
    ) -> Option<*mut u8>,
    pub free: extern "C" fn(
        ctx: *mut c_void,
        memory: *mut u8,
        memory_len: usize,
        alignment_log2: u8,
        ret_addr: usize,
    ),
}

/// Host log callback (`Context.log`). `ctx` is the host's `log_ctx`.
pub type LogFn = extern "C" fn(
    ctx: *const c_void,
    level: LogLevel,
    message_ptr: *const u8,
    message_len: usize,
);

/// Host broker callback (`Context.broker_call`); see broker.rs for the
/// op codes and request/response structs.
pub type BrokerCallFn = extern "C" fn(
    ctx: *mut c_void,
    op: u32,
    req: *const c_void,
    resp: *mut c_void,
) -> u32;

/// The mirrored context. Field order below is the MEASURED physical
/// layout (see module docs); comments give each field's offset and its
/// position in the Zig declaration order.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Context {
    /// offset 0 — declared 1st.
    pub allocator: Allocator,
    /// offset 16 — declared 2nd. Optional slice in the original; the
    /// null pointer is the None encoding.
    pub host_version: StrSlice,
    /// offset 32 — declared 3rd.
    pub log: Option<LogFn>,
    /// offset 40 — declared 4th.
    pub log_ctx: *mut c_void,
    /// offset 48 — declared 7th (moved up by Zig's layout).
    pub broker_call: Option<BrokerCallFn>,
    /// offset 56 — declared 8th (moved up by Zig's layout).
    pub broker_ctx: *mut c_void,
    /// offset 64 — declared 6th.
    pub broker_abi_version: u32,
    /// offset 68 — declared 5th (pushed to the tail by Zig's layout).
    pub json_mode: bool,
}

impl Context {
    /// Mirror of `plugin_api.brokerAvailable`: the broker may only be
    /// used when the callback exists AND its ABI version is >= 1;
    /// otherwise the plugin must take the no-cache path.
    pub fn broker_available(&self) -> bool {
        self.broker_call.is_some() && self.broker_abi_version >= 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    // Values measured on the reference implementation:
    // docs/probes/abi_probe.out.txt.
    #[test]
    fn context_layout_matches_measured_reference() {
        assert_eq!(size_of::<Allocator>(), 16);
        assert_eq!(size_of::<Context>(), 72);
        assert_eq!(offset_of!(Context, allocator), 0);
        assert_eq!(offset_of!(Context, host_version), 16);
        assert_eq!(offset_of!(Context, log), 32);
        assert_eq!(offset_of!(Context, log_ctx), 40);
        assert_eq!(offset_of!(Context, broker_call), 48);
        assert_eq!(offset_of!(Context, broker_ctx), 56);
        assert_eq!(offset_of!(Context, broker_abi_version), 64);
        assert_eq!(offset_of!(Context, json_mode), 68);
    }
}
