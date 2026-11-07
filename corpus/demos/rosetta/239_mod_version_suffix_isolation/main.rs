fn main() {
    let versions = include_str!("versions/index.sa");
    let v1 = include_str!("versions/v1/index.sa");
    let v1_layout = include_str!("versions/v1/layout.sal");
    let v1_seed = include_str!("versions/v1/seed.sa");
    let v2 = include_str!("versions/v2/index.sa");
    let v2_layout = include_str!("versions/v2/layout.sal");
    let v2_seed = include_str!("versions/v2/seed.sa");

    let aggregate_imports_both = versions.contains("versions/v1/index.sa") && versions.contains("versions/v2/index.sa");
    let v1_isolated = v1.contains("versions/v1/layout.sal") && v1_layout.contains("V1_SIZE") && v1_seed.contains("@export v1_seed()");
    let v2_isolated = v2.contains("versions/v2/layout.sal") && v2_layout.contains("V2_SIZE") && v2_seed.contains("@export v2_seed()");
    let no_cross_layout = !v1.contains("V2_") && !v2.contains("V1_");
    let version_isolation = aggregate_imports_both && v1_isolated && v2_isolated && no_cross_layout;

    println!("{}", version_isolation as i32);
}
