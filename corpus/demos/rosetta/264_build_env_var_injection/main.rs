fn main() {
    let config = include_str!("build/env.toml");
    let dev = include_str!("env/dev.env");
    let release = include_str!("env/release.env");
    let generated = include_str!("generated/env_profile.sa");

    let config_selects_release = config.contains("profile = \"release\"")
        && config.contains("env/dev.env")
        && config.contains("env/release.env");
    let env_files_capture_profile = dev.contains("SA_PROFILE=dev")
        && dev.contains("SA_FEATURE_LOGGING=1")
        && release.contains("SA_PROFILE=release")
        && release.contains("SA_FEATURE_LOGGING=0");
    let generated_mentions_env_inputs = generated.contains("build/env.toml")
        && generated.contains("env/*.env")
        && generated.contains("@env_profile_value() -> i32");
    let env_contract = config_selects_release && env_files_capture_profile && generated_mentions_env_inputs;

    println!("{}", env_contract as i32);
}
