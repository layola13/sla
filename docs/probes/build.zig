const std = @import("std");

// Coordinator evidence probes (NOT part of the workspace). Built and run
// ONLY on the Kaggle venue against the venue's own sci / sa_plugin_sla
// trees, so the bytes/sizes measured are the reference implementation's:
//   sleb_probe — sci sab.zig encodeSleb128/encodeUleb128, fixed vectors
//   abi_probe  — sa_plugin_sla plugin_api.zig @sizeOf/@offsetOf table
pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const sab_module = b.addModule("sab", .{
        .root_source_file = .{ .cwd_relative = "/content/sa_all/sci/src/sab.zig" },
        .target = target,
        .optimize = optimize,
    });
    const sleb_module = b.createModule(.{
        .root_source_file = b.path("sleb_main.zig"),
        .target = target,
        .optimize = optimize,
    });
    sleb_module.addImport("sab", sab_module);
    b.installArtifact(b.addExecutable(.{ .name = "sleb_probe", .root_module = sleb_module }));

    const abi_module = b.createModule(.{
        .root_source_file = b.path("abi_main.zig"),
        .target = target,
        .optimize = optimize,
    });
    abi_module.addAnonymousImport("plugin_api", .{
        .root_source_file = .{ .cwd_relative = "/content/sa_all/sa_plugin_sla/src/plugin_api.zig" },
        .target = target,
        .optimize = optimize,
    });
    b.installArtifact(b.addExecutable(.{ .name = "abi_probe", .root_module = abi_module }));

    const dl2_module = b.createModule(.{
        .root_source_file = b.path("dl2_main.zig"),
        .target = target,
        .optimize = optimize,
    });
    dl2_module.addAnonymousImport("lexer_mod", .{
        .root_source_file = .{ .cwd_relative = "/content/sa_all/sa_plugin_sla/src/lexer.zig" },
        .target = target,
        .optimize = optimize,
    });
    b.installArtifact(b.addExecutable(.{ .name = "dl2_probe", .root_module = dl2_module }));
}
