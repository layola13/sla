fn main() {
    let program = include_str!("guest/program.sa");
    let layout = include_str!("guest/program.sal");
    let docs = include_str!("docs/bytecode.md");
    let attach = include_str!("host/trace/attach.txt");
    let pin = include_str!("host/trace/pin.json");

    let program_exports_entry = program.contains("@ffi_wrapper ebpf_gate(*packet: ptr) -> i32")
        && program.contains("@export ebpf_guest_entry() -> i32");
    let layout_defines_packet = layout.contains("#def BpfPacket_kind = +0")
        && layout.contains("#def BpfPacket_value = +4");
    let host_trace_metadata = docs.contains("source, attach metadata, and pinned path")
        && attach.contains("attach = demo-295")
        && attach.contains("program = guest/program.sa")
        && pin.contains("/sys/fs/bpf/demo-295");
    let ebpf_contract = program_exports_entry && layout_defines_packet && host_trace_metadata;

    println!("{}", ebpf_contract as i32);
}
