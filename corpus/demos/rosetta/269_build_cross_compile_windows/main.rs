fn main() {
    let target = include_str!("build/windows/target.toml");
    let generated = include_str!("generated/windows/profile.sa");

    let target_is_windows = target.contains("triple = \"x86_64-pc-windows-msvc\"")
        && target.contains("subsystem = \"console\"");
    let generated_tracks_target = generated.contains("build/windows/target.toml")
        && generated.contains("#def WINDOWS_PROFILE_COUNT = 2")
        && generated.contains("@windows_profile_value() -> i32")
        && generated.contains("@windows_profile_count() -> i32");
    let windows_contract = target_is_windows && generated_tracks_target;

    println!("{}", windows_contract as i32);
}
