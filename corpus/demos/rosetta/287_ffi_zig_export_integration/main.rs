fn main() {
    let guest = include_str!("guest/zig_export.sa");
    let layout = include_str!("guest/zig_export.sal");
    let bridge = include_str!("host/zig/bridge.txt");
    let build_note = include_str!("host/zig/build.zig.note");
    let header = include_str!("host/zig/exported.h");

    let guest_exports_entry = guest.contains("@export zig_entry() -> i32")
        && guest.contains("return ZigExport_VALUE")
        && layout.contains("#def ZigExport_VALUE = 287");
    let zig_host_mentions_import = bridge.contains("Zig host would call the exported SA entrypoint")
        && build_note.contains("Zig build script")
        && header.contains("int zig_entry(void);");
    let zig_contract = guest_exports_entry && zig_host_mentions_import;

    println!("{}", zig_contract as i32);
}
