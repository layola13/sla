const std = @import("std");
const api = @import("plugin_api");

// Coordinator evidence probe for D-C1 (Context is a hand-mirrored plain
// Zig struct, not a C ABI): print the reference implementation's actual
// sizes and field offsets so the Rust #[repr(C)] mirror can be compared
// numerically instead of by assumption.
pub fn main() !void {
    const allocator = std.heap.page_allocator;
    var out: std.Io.Writer.Allocating = .init(allocator);
    defer out.deinit();
    const w = &out.writer;

    try w.print("sizeof Allocator = {d}\n", .{@sizeOf(std.mem.Allocator)});
    try w.print("sizeof Context = {d}\n", .{@sizeOf(api.Context)});
    try w.print("offset Context.allocator = {d}\n", .{@offsetOf(api.Context, "allocator")});
    try w.print("offset Context.host_version = {d}\n", .{@offsetOf(api.Context, "host_version")});
    try w.print("offset Context.log = {d}\n", .{@offsetOf(api.Context, "log")});
    try w.print("offset Context.log_ctx = {d}\n", .{@offsetOf(api.Context, "log_ctx")});
    try w.print("offset Context.json_mode = {d}\n", .{@offsetOf(api.Context, "json_mode")});
    try w.print("offset Context.broker_abi_version = {d}\n", .{@offsetOf(api.Context, "broker_abi_version")});
    try w.print("offset Context.broker_call = {d}\n", .{@offsetOf(api.Context, "broker_call")});
    try w.print("offset Context.broker_ctx = {d}\n", .{@offsetOf(api.Context, "broker_ctx")});
    try w.print("sizeof PluginDescriptor = {d}\n", .{@sizeOf(api.PluginDescriptor)});
    try w.print("offset PluginDescriptor.name = {d}\n", .{@offsetOf(api.PluginDescriptor, "name")});
    try w.print("offset PluginDescriptor.handle_command = {d}\n", .{@offsetOf(api.PluginDescriptor, "handle_command")});
    try w.print("offset PluginDescriptor.skills_ptr = {d}\n", .{@offsetOf(api.PluginDescriptor, "skills_ptr")});
    try w.print("offset PluginDescriptor.skills_len = {d}\n", .{@offsetOf(api.PluginDescriptor, "skills_len")});
    try w.print("sizeof SkillSection = {d}\n", .{@sizeOf(api.SkillSection)});
    try w.print("sizeof HostStream = {d}\n", .{@sizeOf(api.HostStream)});
    try w.print("sizeof BrokerCacheLookupRequest = {d}\n", .{@sizeOf(api.BrokerCacheLookupRequest)});
    try w.print("sizeof BrokerCacheLookupResponse = {d}\n", .{@sizeOf(api.BrokerCacheLookupResponse)});
    try w.print("sizeof BrokerCacheStoreRequest = {d}\n", .{@sizeOf(api.BrokerCacheStoreRequest)});
    try w.print("sizeof BrokerCacheStoreResponse = {d}\n", .{@sizeOf(api.BrokerCacheStoreResponse)});
    std.debug.print("{s}", .{out.written()});
}
