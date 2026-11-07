fn main() {
    let target = include_str!("build/wasm/target.toml");
    let generated = include_str!("generated/wasm/profile.sa");

    let target_is_wasm = target.contains("triple = \"wasm32-unknown-unknown\"")
        && target.contains("linker = \"rust-lld\"");
    let generated_tracks_target = generated.contains("build/wasm/target.toml")
        && generated.contains("#def WASM_PROFILE_COUNT = 2")
        && generated.contains("@wasm_profile_value() -> i32")
        && generated.contains("@wasm_profile_count() -> i32");
    let wasm_contract = target_is_wasm && generated_tracks_target;

    println!("{}", wasm_contract as i32);
}
