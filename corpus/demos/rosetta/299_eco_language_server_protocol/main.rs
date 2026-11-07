fn main() {
    let server = include_str!("lsp/server.sa");
    let layout = include_str!("lsp/server.sal");
    let docs = include_str!("docs/server.md");
    let capabilities = include_str!("host/protocol/capabilities.md");
    let lsp = include_str!("host/protocol/lsp.json");

    let server_exports_entry = server.contains("@ffi_wrapper lsp_server_gate(*state: ptr) -> i32")
        && server.contains("@export lsp_server_entry() -> i32");
    let layout_defines_lsp_state = layout.contains("#def LspState_req = +0")
        && layout.contains("#def LspState_seq = +4");
    let protocol_metadata = docs.contains("wire protocol and client-facing docs")
        && capabilities.contains("protocol surface")
        && lsp.contains("\"jsonrpc\": \"2.0\"")
        && lsp.contains("textDocument/completion");
    let lsp_contract = server_exports_entry && layout_defines_lsp_state && protocol_metadata;

    println!("{}", lsp_contract as i32);
}
