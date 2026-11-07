//! Differential probe (shared part): build the same two logical
//! modules the Rust crate builds in its golden tests, and encode
//! them with the ORIGINAL sci `sab.encodeProgramWithConsts`, so the
//! outputs can be byte-compared.
//!
//! Module A ("minimal") mirrors tests/golden.rs::minimal_module.
//! Module B ("fixture01") mirrors src/fixture01.rs::fixture01_module,
//! field for field (same symbol order, sigs, const decl, operands —
//! including the br operands[2] duplication of Zig `emitBranch`).
//!
//! Two executables (sabprobe_minimal / sabprobe_fixture01) write
//! their module's SAB bytes to stdout; see build.zig. Zig 0.17
//! removed std.fs.cwd() and std.process.argsAlloc, so output goes
//! through std.posix.write on fd 1 and mode selection is by binary.

const std = @import("std");
const sab = @import("sab");
const inst = sab.instruction;
const sig = sab.signature;
const cd = sab.const_decl;

pub fn writeOut(bytes: []const u8) !void {
    // Zig 0.17 removed std.fs.cwd(), std.process.argsAlloc and
    // std.posix.write alike; the raw linux syscall wrapper is the
    // stable way to put bytes on fd 1.
    var written: usize = 0;
    while (written < bytes.len) {
        const rc = std.os.linux.write(1, bytes.ptr + written, bytes.len - written);
        if (std.os.linux.errno(rc) != .SUCCESS) return error.WriteFailed;
        written += rc;
    }
    std.debug.print("wrote {d} bytes to stdout\n", .{bytes.len});
}

pub fn encodeMinimal(allocator: std.mem.Allocator) ![]u8 {
    const symbols = [_][]const u8{"f"};
    var func_decl = inst.makeInstruction(.func_decl, 0, 0, null, "");
    func_decl.operands[0] = .{ .symbol = 0 };
    func_decl.operands[1] = .{ .func = 0 };
    const ret = inst.makeInstruction(.return_, 0, 1, null, "");
    const insts = [_]inst.Instruction{ func_decl, ret };
    const sigs = [_]sig.FunctionSig{.{
        .id = 0,
        .name = "f",
        .params = &.{},
        .kind = .normal,
        .return_cap = null,
        .return_ty = .void,
        .entry_inst_idx = 0,
        .is_ffi_wrapper = false,
    }};
    return sab.encodeProgram(allocator, &symbols, &sigs, &insts);
}

pub fn encodeFixture01(allocator: std.mem.Allocator) ![]u8 {
    const symbols = [_][]const u8{
        "main", "SLA_STR_0", "tmp_0", "test_hello_world",
        "tmp_1", "then_0", "else_1",
    };
    var insts: [11]inst.Instruction = undefined;
    insts[0] = inst.makeInstruction(.func_decl, 0, 0, null, "");
    insts[0].operands[0] = .{ .symbol = 0 };
    insts[0].operands[1] = .{ .func = 0 };
    insts[1] = inst.makeInstruction(.call, 0, 1, null, "");
    insts[1].operands[1] = .{ .text = "@println(SLA_STR_0)" };
    insts[2] = inst.makeInstruction(.assign, 0, 2, null, "");
    insts[2].operands[0] = .{ .reg = 2 };
    insts[2].operands[1] = .{ .imm_i64 = 0 };
    insts[3] = inst.makeInstruction(.return_, 0, 3, null, "");
    insts[3].operands[0] = .{ .reg = 2 };
    insts[4] = inst.makeInstruction(.test_decl, 0, 4, null, "");
    insts[4].operands[0] = .{ .symbol = 3 };
    insts[4].operands[1] = .{ .func = 3 };
    insts[5] = inst.makeInstruction(.call, 0, 5, null, "");
    insts[5].operands[0] = .{ .reg = 4 };
    insts[5].operands[1] = .{ .text = "@str_eq(SLA_STR_0, SLA_STR_0)" };
    insts[6] = inst.makeInstruction(.br, 0, 6, null, "");
    insts[6].operands[0] = .{ .reg = 4 };
    insts[6].operands[1] = .{ .label = 5 };
    insts[6].operands[2] = .{ .label = 5 };
    insts[6].operands[3] = .{ .label = 6 };
    insts[7] = inst.makeInstruction(.label, 0, 7, null, "");
    insts[7].operands[0] = .{ .symbol = 6 };
    insts[7].operands[1] = .{ .label = 6 };
    insts[8] = inst.makeInstruction(.panic, 0, 8, null, "");
    insts[8].operands[0] = .{ .text = "101" };
    insts[9] = inst.makeInstruction(.label, 0, 9, null, "");
    insts[9].operands[0] = .{ .symbol = 5 };
    insts[9].operands[1] = .{ .label = 5 };
    insts[10] = inst.makeInstruction(.return_, 0, 10, null, "");

    const sigs = [_]sig.FunctionSig{
        .{
            .id = 0,
            .name = "main",
            .params = &.{},
            .kind = .normal,
            .return_cap = null,
            .return_ty = .i32,
            .entry_inst_idx = 0,
            .is_ffi_wrapper = false,
            .reg_ids = &.{2},
        },
        .{
            .id = 3,
            .name = "test_hello_world",
            .params = &.{},
            .kind = .test_func,
            .return_cap = null,
            .return_ty = .void,
            .entry_inst_idx = 4,
            .is_ffi_wrapper = false,
            .reg_ids = &.{4},
        },
    };

    const hello = try allocator.dupe(u8, "hello world\x00");
    const consts = [_]cd.ConstDecl{.{
        .source_line = 0,
        .expanded_line = 0,
        .upstream_loc = null,
        .raw_text = try allocator.dupe(u8, "@const SLA_STR_0 = utf8:\"hello world\\0\""),
        .name = try allocator.dupe(u8, "SLA_STR_0"),
        .literal_text = try allocator.dupe(u8, "utf8:\"hello world\\0\""),
        .value = .{ .utf8 = .{ .kind = .utf8, .bytes = hello } },
    }};

    return sab.encodeProgramWithConsts(allocator, &symbols, &consts, &sigs, &insts);
}
