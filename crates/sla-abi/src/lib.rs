//! sla-abi: the host (sci) <-> SLA plugin C ABI surface, mirrored from
//! `sa_plugin_sla/src/plugin_api.zig` and `plugin_handler.zig`.
//!
//! Zero-dependency leaf crate (six-house verdict 2026-11-07): it carries
//! ONLY the ABI types, constants and signatures — the exported anchor
//! symbols live in `sla-plugin`, never here.
//!
//! Mirror discipline: `PluginDescriptor`, `HostStream` and the handler /
//! broker structs are `extern struct` in Zig (true C layout) and are
//! mirrored field-for-field with size/offset tests pinning the measured
//! values. Nullable Zig pointers (`?[*]T`, `?*anyopaque`) are mirrored
//! as PLAIN Rust raw pointers (null = absent), never as `Option<*mut T>`:
//! Rust gives `Option` of a raw pointer a discriminant (16 B), which
//! silently inflates every struct containing one — the first version of
//! this mirror made exactly that mistake and the layout tests caught it
//! (Context 88 vs measured 72). `Option` is used only for function
//! pointers, whose null niche is real. `Context` is NOT a C ABI in the original — it is a plain Zig
//! struct hand-mirrored on both sides (divergence D-C1); its Rust mirror
//! follows the layout MEASURED on the reference implementation
//! (docs/probes/abi_probe.out.txt), not the Zig declaration order,
//! because Zig's auto layout reorders fields by alignment.
//!
//! Not mirrored: `plugin_api.Plugin` (a Zig-internal convenience struct
//! whose function pointers return `anyerror!` unions — it never crosses
//! the C boundary) and the `emitLog` helper (its behavior is reproduced
//! by callers through `Context::log`).

pub mod broker;
pub mod context;
pub mod descriptor;
pub mod handler;

pub use broker::{
    BrokerCacheLookupRequest, BrokerCacheLookupResponse, BrokerCacheStoreRequest,
    BrokerCacheStoreResponse, BROKER_OP_CACHE_LOOKUP, BROKER_OP_CACHE_STORE,
};
pub use context::{Allocator, AllocatorVTable, BrokerCallFn, Context, LogFn};
pub use descriptor::{
    DescriptorFn, HandleCommandFn, HostStream, InitFn, PluginDescriptor, PostbuildFn,
    PrebuildFn, SkillSection, StreamWriteAllFn, StrSlice, StrSliceList,
};
pub use handler::{
    SlaCompileHandlerOptionsAbi, SlaCompileHandlerResultAbi, SlaHandlerStateFieldAbi,
    HANDLER_TY_F64, HANDLER_TY_I1, HANDLER_TY_I32, HANDLER_TY_I64, HANDLER_TY_PTR,
};

/// `plugin_api.abi_version` — the only descriptor version the host accepts.
pub const ABI_VERSION: u32 = 1;

/// `plugin_api.host_version`.
pub const HOST_VERSION: &str = "sci-0.1";

/// `plugin_api.descriptor_symbol_name` — data symbol the host looks up
/// first; `saasm_plugin_descriptor_v1_fn` is the fallback function form.
pub const DESCRIPTOR_SYMBOL_NAME: &str = "saasm_plugin_descriptor_v1";

/// Return code of `handle_command` and of the stream write callback
/// (`plugin_api.AbiStatus`). The host turns `Failed`, `VersionMismatch`
/// and `InvalidDescriptor` into diagnostics.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiStatus {
    Ok = 0,
    UnknownCommand = 1,
    Failed = 2,
    VersionMismatch = 3,
    InvalidDescriptor = 4,
}

impl AbiStatus {
    pub fn code(self) -> u32 {
        self as u32
    }
}

/// Host log severity (`plugin_api.LogLevel`, `enum(u8)`).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug = 0,
    Info = 1,
    Warn = 2,
    Err = 3,
}
