//! sla-plugin: cdylib top level — the exported descriptor anchors and
//! (eventually) the pipeline wiring. Nobody depends on this crate.
//!
//! State, honestly: the descriptor pair
//! (`saasm_plugin_descriptor_v1` data + `saasm_plugin_descriptor_v1_fn`)
//! is exported with the same field values as the Zig original
//! (plugin.zig's static descriptor over plugin_skills.zig's table).
//! `handle_command` dispatches through `commands::dispatch`: the A0
//! slice — the commands that do not drive the compile pipeline
//! (`help`, `init`, `skills`, `stability`, the `sab` dispatcher's
//! help/error paths, and every command's `-h/--help` and
//! option-error paths) — is implemented for real, mirrored from
//! plugin_commands.zig / plugin_cli.zig at the venue head. Pipeline
//! commands (`build`, `check`, `test`, the SAB builders, …) still
//! return `AbiStatus::Failed` once their options parse — loud by
//! design; a silent `Ok` there would fake a working compiler. The
//! second ABI (`sla_compile_handler` /
//! `sla_compile_handler_result_free`) is NOT exported yet: it belongs
//! to the handler compiler, which has no port yet, and fabricating a
//! stub there would invent behavior the original does not have.

pub mod cli_help;
pub mod cmdio;
pub mod commands;
pub mod contracts;
pub mod init_cmd;
pub mod skills_cmd;
pub mod sla_expand;
pub mod stability;
pub mod stability_validate;

use sla_abi::{
    AbiStatus, Context, HandleCommandFn, HostStream, PluginDescriptor, SkillSection,
    StrSlice, StrSliceList, ABI_VERSION,
};
use std::ffi::c_char;

pub const PLUGIN_NAME: &str = "sla";

const NAME_CSTR: &[u8] = b"sla\0";

/// The one skills section, mirroring plugin_skills.zig verbatim
/// (name / summary / the 14 usage strings, in order).
static SKILL_ITEMS: [StrSlice; 14] = [
    StrSlice::from_static_str("sla init [path]"),
    StrSlice::from_static_str("sla skills [--json]"),
    StrSlice::from_static_str("sla stability schema|verify ..."),
    StrSlice::from_static_str("sla build [file] [-p <package>] [--out <file>]"),
    StrSlice::from_static_str("sla build-workspace [-p <package>] [sa-build-exe-options...]"),
    StrSlice::from_static_str("sla build-exe [file] [-p <package>] [sa-build-exe-options...]"),
    StrSlice::from_static_str("sla build-wasm [file] [-p <package>] [sa-build-wasm-options...]"),
    StrSlice::from_static_str("sla build-js [file] [-p <package>] [sa-build-js-options...]"),
    StrSlice::from_static_str("sla build-c [file] [-p <package>] [sa-build-c-options...]"),
    StrSlice::from_static_str("sla sab build [file] [-p <package>] [--out <file.sab>]"),
    StrSlice::from_static_str(
        "sla sab workspace [-p <package>] [--sab-out <file.sab>] [sa-build-exe-options...]",
    ),
    StrSlice::from_static_str("slab build|workspace|disasm ..."),
    StrSlice::from_static_str("sla check [file] [-p <package>]"),
    StrSlice::from_static_str(
        "sla test [file] [-p <package>] [--test-backend auto|sab|sa] [sa-test-options...]",
    ),
];

static SKILLS: [SkillSection; 1] = [SkillSection {
    name: StrSlice::from_static_str("sla"),
    summary: StrSlice::from_static_str("Sla compiler and tools"),
    items: StrSliceList::from_static_slice(&SKILL_ITEMS),
}];

/// Command entry, mirroring `runSlaCommandAbi` (plugin.zig): the
/// dispatch result maps onto (status, out_code) as — a command code
/// -> (Ok, code); an unknown command -> (UnknownCommand, 0); an
/// invocation failure -> (Failed, 0). Note the failure path writes
/// out_code 0, exactly as plugin.zig does; the phase-1 stub wrote 1
/// there, which no Zig code path produces.
extern "C" fn run_sla_command_abi(
    ctx: *const Context,
    argv: *const *const c_char,
    argv_len: usize,
    stdout: HostStream,
    stderr: HostStream,
    out_code: *mut u8,
) -> u32 {
    if !out_code.is_null() {
        // SAFETY: the host always passes a writable out_code slot.
        unsafe { *out_code = 0 };
    }
    let json_mode = if ctx.is_null() {
        false
    } else {
        // SAFETY: the host hands in a live Context for the call.
        unsafe { (*ctx).json_mode }
    };
    // Decode argv exactly as the host built it: argv_len
    // NUL-terminated strings (program name first).
    let mut args: Vec<Vec<u8>> = Vec::with_capacity(argv_len);
    if argv.is_null() && argv_len > 0 {
        return AbiStatus::Failed.code();
    }
    for i in 0..argv_len {
        // SAFETY: argv/argv[i] follow the host's argv contract.
        let arg_ptr = unsafe { *argv.add(i) };
        if arg_ptr.is_null() {
            return AbiStatus::Failed.code();
        }
        let bytes = unsafe { std::ffi::CStr::from_ptr(arg_ptr) }.to_bytes();
        args.push(bytes.to_vec());
    }
    let mut out_sink = cmdio::HostSink(stdout);
    let mut err_sink = cmdio::HostSink(stderr);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        commands::dispatch(json_mode, &args, &mut out_sink, &mut err_sink)
    }));
    match result {
        Ok(Ok(commands::DispatchResult::Code(code))) => {
            if !out_code.is_null() {
                // SAFETY: as above.
                unsafe { *out_code = code };
            }
            AbiStatus::Ok.code()
        }
        Ok(Ok(commands::DispatchResult::Unknown)) => AbiStatus::UnknownCommand.code(),
        Ok(Ok(commands::DispatchResult::Fail)) | Ok(Err(())) | Err(_) => {
            AbiStatus::Failed.code()
        }
    }
}

const HANDLE_COMMAND: HandleCommandFn = run_sla_command_abi;

const DESCRIPTOR: PluginDescriptor = PluginDescriptor {
    abi_version: ABI_VERSION,
    descriptor_size: std::mem::size_of::<PluginDescriptor>() as u32,
    name: NAME_CSTR.as_ptr() as *const c_char,
    init: None,
    prebuild: None,
    postbuild: None,
    handle_command: Some(HANDLE_COMMAND),
    skills_ptr: SKILLS.as_ptr(),
    skills_len: 1,
};

/// The static descriptor lives behind a transparent cell because
/// `PluginDescriptor` (raw pointers) is not `Sync`; the byte layout
/// seen by the host is exactly `PluginDescriptor`'s.
#[repr(transparent)]
pub struct DescriptorCell(PluginDescriptor);

unsafe impl Sync for DescriptorCell {}

/// Data-form descriptor anchor — the host's first lookup
/// (`loadDescriptor` prefers the data symbol, per docs/ABI_MAP.md §1).
#[no_mangle]
pub static saasm_plugin_descriptor_v1: DescriptorCell = DescriptorCell(DESCRIPTOR);

/// Function-form descriptor anchor — the host's fallback: fills `out`
/// with a copy of the static descriptor.
#[no_mangle]
pub extern "C" fn saasm_plugin_descriptor_v1_fn(out: *mut PluginDescriptor) {
    if !out.is_null() {
        // SAFETY: the host passes a writable PluginDescriptor slot.
        unsafe { *out = DESCRIPTOR };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    fn filled() -> PluginDescriptor {
        let mut d = PluginDescriptor {
            abi_version: 0,
            descriptor_size: 0,
            name: std::ptr::null(),
            init: None,
            prebuild: None,
            postbuild: None,
            handle_command: None,
            skills_ptr: std::ptr::null(),
            skills_len: 0,
        };
        saasm_plugin_descriptor_v1_fn(&mut d);
        d
    }

    #[test]
    fn descriptor_matches_zig_static() {
        let d = filled();
        assert_eq!(d.abi_version, 1);
        assert_eq!(d.descriptor_size, 64);
        assert!(d.init.is_none() && d.prebuild.is_none() && d.postbuild.is_none());
        assert!(d.handle_command.is_some());
        assert_eq!(d.skills_len, 1);
        // SAFETY: the descriptor points at this crate's own statics.
        let name = unsafe { CStr::from_ptr(d.name) };
        assert_eq!(name.to_bytes(), b"sla");
        let section = unsafe { &*d.skills_ptr };
        let sec_name = unsafe { section.name.as_bytes() };
        assert_eq!(sec_name, b"sla");
        let summary = unsafe { section.summary.as_bytes() };
        assert_eq!(summary, b"Sla compiler and tools");
        assert_eq!(section.items.len, 14);
        let items = unsafe {
            std::slice::from_raw_parts(section.items.ptr, section.items.len)
        };
        let first = unsafe { items[0].as_bytes() };
        let last = unsafe { items[13].as_bytes() };
        assert_eq!(first, b"sla init [path]");
        assert!(last.starts_with(b"sla test [file]"));
    }

    #[test]
    fn data_anchor_agrees_with_fn_anchor() {
        let d = filled();
        assert_eq!(saasm_plugin_descriptor_v1.0.abi_version, d.abi_version);
        assert_eq!(saasm_plugin_descriptor_v1.0.skills_len, d.skills_len);
        assert_eq!(
            saasm_plugin_descriptor_v1.0.skills_ptr, d.skills_ptr,
            "both anchors must describe the same statics"
        );
    }
}
