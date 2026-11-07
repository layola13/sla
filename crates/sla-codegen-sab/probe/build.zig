const std = @import("std");

// Probe build: wire sci's sab.zig (venue path /content/sa_all/sci) as
// the "sab" module. Only std + relative common/* imports are needed
// for the encode path. Builds two executables sharing common.zig:
//   zig build && ./zig-out/bin/sabprobe_minimal   > minimal_zig.sab
//               && ./zig-out/bin/sabprobe_fixture01 > fixture01_zig.sab
pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});
    const sab_module = b.addModule("sab", .{
        .root_source_file = .{ .cwd_relative = "/content/sa_all/sci/src/sab.zig" },
        .target = target,
        .optimize = optimize,
    });
    for ([_]struct { name: []const u8, root: []const u8 }{
        .{ .name = "sabprobe_minimal", .root = "main_minimal.zig" },
        .{ .name = "sabprobe_fixture01", .root = "main_fixture01.zig" },
    }) |spec| {
        const exe_module = b.createModule(.{
            .root_source_file = b.path(spec.root),
            .target = target,
            .optimize = optimize,
        });
        exe_module.addImport("sab", sab_module);
        const exe = b.addExecutable(.{ .name = spec.name, .root_module = exe_module });
        b.installArtifact(exe);
    }
}
