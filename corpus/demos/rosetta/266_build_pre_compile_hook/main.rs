fn main() {
    let config = include_str!("build/pre-hooks.toml");
    let script = include_str!("hooks/pre-compile.sh");
    let notes = include_str!("hooks/pre-compile.txt");
    let generated = include_str!("generated/precompile.sa");

    let config_points_to_hook = config.contains("pre_compile = \"hooks/pre-compile.sh\"")
        && config.contains("output = \"generated/precompile.sa\"");
    let hook_script_mentions_generation = script.contains("prepare generated/precompile.sa")
        && script.contains("pre-compile hook prepares inputs before SA-ASM compilation");
    let hook_notes_match = notes.contains("pre-compile hook")
        && notes.contains("prepare source metadata")
        && notes.contains("emit generated/precompile.sa");
    let generated_mentions_hook = generated.contains("hooks/pre-compile.sh")
        && generated.contains("build/pre-hooks.toml")
        && generated.contains("@precompile_value() -> i32");
    let precompile_contract = config_points_to_hook && hook_script_mentions_generation && hook_notes_match && generated_mentions_hook;

    println!("{}", precompile_contract as i32);
}
