const std = @import("std");
const lexer = @import("lexer_mod");

// Coordinator evidence probe for D-L2 (lexer escape-at-EOF overshoot):
// drive the ORIGINAL lexer over buffers whose last byte is a backslash
// inside an unterminated string / template, and print every token's
// loc against the buffer length. Control cases (plain unterminated,
// escaped backslash pair) are included.
const Lexer = lexer.Lexer;

fn runCase(w: *std.Io.Writer, label: []const u8, buffer: []const u8) !void {
    try w.print("case {s}: buffer_len={d}\n", .{ label, buffer.len });
    var lx = Lexer.init(buffer);
    var i: usize = 0;
    while (i < 32) : (i += 1) {
        const tok = lx.next();
        try w.print("  tok {s} loc=[{d}..{d}]\n", .{ @tagName(tok.tag), tok.loc.start, tok.loc.end });
        if (tok.tag == .eof) break;
    }
}

pub fn main() !void {
    const allocator = std.heap.page_allocator;
    var out: std.Io.Writer.Allocating = .init(allocator);
    defer out.deinit();
    const w = &out.writer;

    try runCase(w, "string_eof_backslash", "\"abc\\");
    try runCase(w, "string_eof_plain", "\"abc");
    try runCase(w, "string_eof_escaped_pair", "\"abc\\\\");
    try runCase(w, "template_eof_backslash", "`abc\\");
    try runCase(w, "stmt_string_eof_backslash", "let s = \"abc\\");
    std.debug.print("{s}", .{out.written()});
}
