fn main() {
    let public = include_str!("public/index.sa");
    let bridge = include_str!("internal/bridge.sa");
    let seed = include_str!("internal/detail/seed.sa");

    let public_wraps_internal = public.contains("public/../internal/bridge.sa") && public.contains("@export public_value()");
    let internal_imports_detail = bridge.contains("internal/detail/seed.sa") && bridge.contains("@export internal_value()");
    let private_detail_exists = seed.contains("@export private_seed()");
    let public_file_avoids_detail = !public.contains("internal/detail/seed.sa") && !public.contains("private_seed");
    let visibility_layout = public_wraps_internal && internal_imports_detail && private_detail_exists && public_file_avoids_detail;

    println!("{}", visibility_layout as i32);
}
