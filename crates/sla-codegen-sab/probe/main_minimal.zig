const std = @import("std");
const common = @import("common.zig");

pub fn main() !void {
    var arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);
    const bytes = try common.encodeMinimal(arena.allocator());
    try common.writeOut(bytes);
}
