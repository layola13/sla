fn main() {
    let config = include_str!("build/post-hooks.toml");
    let script = include_str!("hooks/post-compile.sh");
    let manifest = include_str!("artifacts/post-build/manifest.json");
    let report = include_str!("artifacts/post-build/report.txt");
    let generated = include_str!("generated/postcompile.sa");

    let config_points_to_post_hook = config.contains("post_compile = \"hooks/post-compile.sh\"")
        && config.contains("artifact_manifest = \"artifacts/post-build/manifest.json\"")
        && config.contains("output = \"generated/postcompile.sa\"");
    let post_hook_writes_report = script.contains("collect artifacts/post-build/report.txt")
        && script.contains("writes reports after the SA-ASM output exists");
    let artifacts_are_real = manifest.contains("\"stage\": \"post-compile\"")
        && manifest.contains("generated/postcompile.sa")
        && report.contains("post-compile report for 267");
    let generated_mentions_post_hook = generated.contains("hooks/post-compile.sh")
        && generated.contains("build/post-hooks.toml")
        && generated.contains("@postcompile_value() -> i32");
    let postcompile_contract = config_points_to_post_hook && post_hook_writes_report && artifacts_are_real && generated_mentions_post_hook;

    println!("{}", postcompile_contract as i32);
}
