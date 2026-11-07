//! Descriptor-side ABI types: `PluginDescriptor`, `SkillSection`,
//! `HostStream` and the string-slice building blocks.
//!
//! All are `extern struct` in the Zig original (see docs/ABI_MAP.md §2/§4)
//! except `SkillSection`, which is a plain struct whose measured size
//! (48 B, three ptr+len slices in declaration order) the `#[repr(C)]`
//! mirror reproduces exactly — pinned by the tests below against
//! docs/probes/abi_probe.out.txt.

use crate::{AbiStatus, Context};
use std::ffi::c_char;
use std::os::raw::c_void;

/// A Zig `[]const u8` at the ABI boundary: pointer + length, NOT
/// NUL-terminated. A null pointer denotes the empty slice in the
/// original's convention (see divergence D-C2 for the places where the
/// original does not distinguish null from empty).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StrSlice {
    pub ptr: *const u8,
    pub len: usize,
}

impl StrSlice {
    pub const fn from_static_str(s: &'static str) -> StrSlice {
        StrSlice {
            ptr: s.as_ptr(),
            len: s.len(),
        }
    }

    pub const fn empty() -> StrSlice {
        StrSlice {
            ptr: b"".as_ptr(),
            len: 0,
        }
    }

    pub fn is_null(&self) -> bool {
        self.ptr.is_null()
    }

    /// Borrow the bytes. Only sound for slices produced by the host or
    /// by this plugin's statics during the call they were handed over in.
    ///
    /// # Safety
    /// `ptr`/`len` must describe a readable byte string of `len` bytes,
    /// or `ptr` must be null (treated as empty).
    pub unsafe fn as_bytes(&self) -> &[u8] {
        if self.ptr.is_null() {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
        }
    }
}

/// A Zig `[]const []const u8`: a slice of string slices.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StrSliceList {
    pub ptr: *const StrSlice,
    pub len: usize,
}

impl StrSliceList {
    pub const fn from_static_slice(items: &'static [StrSlice]) -> StrSliceList {
        StrSliceList {
            ptr: items.as_ptr(),
            len: items.len(),
        }
    }

    pub const fn empty() -> StrSliceList {
        StrSliceList {
            ptr: std::ptr::null(),
            len: 0,
        }
    }
}

/// One section of the descriptor's skills array (`plugin_api.SkillSection`).
/// The SLA plugin publishes exactly one section, named "sla".
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SkillSection {
    pub name: StrSlice,
    pub summary: StrSlice,
    pub items: StrSliceList,
}

// The slice types live in plugin statics, so they must be Sync. Sharing
// a (ptr, len) pair across threads for READS is sound — the same
// rationale as `&str`'s Sync; writing through the pointer is governed
// by the unsafe contract on `StrSlice::as_bytes`, as in the original.
unsafe impl Sync for StrSlice {}
unsafe impl Sync for StrSliceList {}
unsafe impl Sync for SkillSection {}

/// Signature of the host stream write callback (`HostStream.write_all`).
/// Returns an `AbiStatus` code; anything but `Ok` aborts the write in
/// the original's sink wrapper.
pub type StreamWriteAllFn =
    extern "C" fn(ctx: *mut c_void, bytes: *const u8, len: usize) -> u32;

/// A host output stream, passed BY VALUE into `handle_command`
/// (`plugin_api.HostStream`, 16 B).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct HostStream {
    pub ctx: *mut c_void,
    pub write_all: Option<StreamWriteAllFn>,
}

impl HostStream {
    /// Write all bytes through the host callback.
    ///
    /// # Safety
    /// The stream must be one handed in by the host for the current call.
    pub unsafe fn write_all(&self, bytes: &[u8]) -> Result<(), AbiStatus> {
        let write = self.write_all.ok_or(AbiStatus::Failed)?;
        let rc = write(self.ctx, bytes.as_ptr(), bytes.len());
        if rc == AbiStatus::Ok.code() {
            Ok(())
        } else {
            Err(AbiStatus::Failed)
        }
    }
}

/// `PluginDescriptor.init`: called by the host right after validation.
pub type InitFn = extern "C" fn(ctx: *const Context) -> u32;
/// `PluginDescriptor.prebuild`.
pub type PrebuildFn =
    extern "C" fn(ctx: *const Context, compile_options: *mut c_void) -> u32;
/// `PluginDescriptor.postbuild`.
pub type PostbuildFn = extern "C" fn(ctx: *const Context) -> u32;
/// `PluginDescriptor.handle_command` — see docs/ABI_MAP.md §3 for the
/// argv convention and the `out_code` contract.
pub type HandleCommandFn = extern "C" fn(
    ctx: *const Context,
    argv: *const *const c_char,
    argv_len: usize,
    stdout: HostStream,
    stderr: HostStream,
    out_code: *mut u8,
) -> u32;
/// The descriptor function form (`saasm_plugin_descriptor_v1_fn`):
/// fills `out` with a copy of the static descriptor.
pub type DescriptorFn = extern "C" fn(out: *mut PluginDescriptor);

/// The plugin descriptor (`plugin_api.PluginDescriptor`, extern struct,
/// 64 B). The host validates `abi_version` and `descriptor_size` before
/// anything else; `name` must be a NUL-terminated static string.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PluginDescriptor {
    pub abi_version: u32,
    pub descriptor_size: u32,
    pub name: *const c_char,
    pub init: Option<InitFn>,
    pub prebuild: Option<PrebuildFn>,
    pub postbuild: Option<PostbuildFn>,
    pub handle_command: Option<HandleCommandFn>,
    pub skills_ptr: *const SkillSection,
    pub skills_len: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    // Values measured on the reference implementation:
    // docs/probes/abi_probe.out.txt.
    #[test]
    fn descriptor_layout_matches_reference() {
        assert_eq!(size_of::<PluginDescriptor>(), 64);
        assert_eq!(offset_of!(PluginDescriptor, abi_version), 0);
        assert_eq!(offset_of!(PluginDescriptor, descriptor_size), 4);
        assert_eq!(offset_of!(PluginDescriptor, name), 8);
        assert_eq!(offset_of!(PluginDescriptor, init), 16);
        assert_eq!(offset_of!(PluginDescriptor, prebuild), 24);
        assert_eq!(offset_of!(PluginDescriptor, postbuild), 32);
        assert_eq!(offset_of!(PluginDescriptor, handle_command), 40);
        assert_eq!(offset_of!(PluginDescriptor, skills_ptr), 48);
        assert_eq!(offset_of!(PluginDescriptor, skills_len), 56);
    }

    #[test]
    fn skill_section_and_stream_layouts_match_reference() {
        assert_eq!(size_of::<SkillSection>(), 48);
        assert_eq!(offset_of!(SkillSection, name), 0);
        assert_eq!(offset_of!(SkillSection, summary), 16);
        assert_eq!(offset_of!(SkillSection, items), 32);
        assert_eq!(size_of::<HostStream>(), 16);
        assert_eq!(offset_of!(HostStream, ctx), 0);
        assert_eq!(offset_of!(HostStream, write_all), 8);
        assert_eq!(size_of::<StrSlice>(), 16);
        assert_eq!(size_of::<StrSliceList>(), 16);
    }
}
