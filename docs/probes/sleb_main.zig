const std = @import("std");
const sab = @import("sab");

// Coordinator evidence probe for the sleb128 known-vector dispute
// (COORDINATION.md §6): run the ORIGINAL sci encoder over the disputed
// inputs and print the bytes it actually emits.
const hexdigits = "0123456789abcdef";

fn putHex(w: *std.Io.Writer, bytes: []const u8) !void {
    for (bytes) |byte| {
        try w.writeByte(hexdigits[byte >> 4]);
        try w.writeByte(hexdigits[byte & 0x0f]);
        try w.writeByte(' ');
    }
}

pub fn main() !void {
    const allocator = std.heap.page_allocator;
    var out: std.Io.Writer.Allocating = .init(allocator);
    defer out.deinit();
    const w = &out.writer;

    const sleb_vals = [_]i64{ 0, 1, -1, 63, 64, -64, -65, -12345, -12365, 12345, -1000000, 1000000 };
    for (sleb_vals) |v| {
        var buf: std.Io.Writer.Allocating = .init(allocator);
        defer buf.deinit();
        try sab.encodeSleb128(&buf.writer, v);
        try w.print("sleb {d} =", .{v});
        try putHex(w, buf.written());
        try w.writeByte('\n');
    }
    const uleb_vals = [_]u64{ 0, 1, 63, 128, 300, 16384, 4294967295 };
    for (uleb_vals) |v| {
        var buf: std.Io.Writer.Allocating = .init(allocator);
        defer buf.deinit();
        try sab.encodeUleb128(&buf.writer, v);
        try w.print("uleb {d} =", .{v});
        try putHex(w, buf.written());
        try w.writeByte('\n');
    }
    std.debug.print("{s}", .{out.written()});
}
