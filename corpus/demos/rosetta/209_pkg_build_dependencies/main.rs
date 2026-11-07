fn main() {
    let pkg = include_str!("sa.pkg");
    let src = include_str!("src/index.sa");
    let generated = include_str!("build/generated/index.sa");
    let generated_consts = include_str!("build/generated/generated.sal");

    let declares_build_dep = pkg.contains("build-dependencies") && pkg.contains("build/generated");
    let src_imports_generated = src.contains("build/generated/index.sa");
    let generated_uses_artifact = generated.contains("generated.sal") && generated.contains("@generated_artifact_offset()");
    let artifact_value = generated_consts.contains("BUILD_ARTIFACT_OFFSET = 9");
    let result = declares_build_dep && src_imports_generated && generated_uses_artifact && artifact_value;

    println!("{}", result as i32);
}
