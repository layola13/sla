fn main() {
    let publish = include_str!("registry/publish.sa");
    let layout = include_str!("registry/publish.sal");
    let docs = include_str!("docs/publish.md");
    let registry = include_str!("host/registry/registry.json");
    let log = include_str!("host/registry/publish.log");

    let publish_exports_entry = publish.contains("@ffi_wrapper registry_publish_gate(*pkg: ptr) -> i32")
        && publish.contains("@export registry_publish_entry() -> i32");
    let layout_defines_package = layout.contains("#def Package_name = +0")
        && layout.contains("#def Package_version = +4");
    let registry_metadata = docs.contains("registry publish step")
        && registry.contains("\"registry\": \"sa-lang.org\"")
        && registry.contains("\"package\": \"demo-300\"")
        && log.contains("publish demo-300 to sa-lang.org/registry");
    let registry_contract = publish_exports_entry && layout_defines_package && registry_metadata;

    println!("{}", registry_contract as i32);
}
