fn main() {
    let config = include_str!("bindgen/bindgen.toml");
    let header_api = include_str!("bindgen/include/ffi_api.h");
    let header_platform = include_str!("bindgen/include/ffi_platform.h");
    let generated = include_str!("generated/bindings.sa");

    let config_lists_headers = config.contains("bindgen/include/ffi_api.h")
        && config.contains("bindgen/include/ffi_platform.h")
        && config.contains("output = \"generated/bindings.sa\"");
    let headers_define_version_and_name = header_api.contains("#define FFI_API_VERSION 262")
        && header_api.contains("int ffi_api_version(void);")
        && header_platform.contains("#define FFI_PLATFORM_NAME \"saasm-bindgen\"");
    let generated_mentions_headers = generated.contains("bindgen/bindgen.toml")
        && generated.contains("bindgen/include/*.h")
        && generated.contains("@bindgen_value() -> i32");
    let bindgen_contract = config_lists_headers && headers_define_version_and_name && generated_mentions_headers;

    println!("{}", bindgen_contract as i32);
}
