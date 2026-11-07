fn main() {
    let build_config = include_str!("build/sanitizer.toml");
    let flags_config = include_str!("config/sanitizer/flags.toml");
    let generated = include_str!("generated/sanitizer/flags.sa");

    let build_requests_sanitizers = build_config.contains("address")
        && build_config.contains("undefined");
    let flags_enable_both = flags_config.contains("address = true")
        && flags_config.contains("undefined = true");
    let generated_counts_flags = generated.contains("build/sanitizer.toml")
        && generated.contains("config/sanitizer/flags.toml")
        && generated.contains("#def SANITIZER_FLAG_COUNT = 2");
    let sanitizer_contract = build_requests_sanitizers && flags_enable_both && generated_counts_flags;

    println!("{}", sanitizer_contract as i32);
}
