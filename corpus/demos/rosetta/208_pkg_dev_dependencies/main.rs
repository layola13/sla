fn main() {
    let pkg = include_str!("sa.pkg");
    let main_src = include_str!("src/index.sa");
    let dev_test = include_str!("dev/tests/index.sa");
    let dev_helper = include_str!("dev/helpers/index.sa");

    let declares_dev_dep = pkg.contains("dev-dependencies") && pkg.contains("dev/helpers");
    let release_path_avoids_dev = !main_src.contains("dev/");
    let dev_path_reuses_helper_flags = dev_helper.contains("src/helpers/flags.sal");
    let dev_tests_use_dev_helpers = dev_test.contains("dev/tests/../helpers/index.sa");
    let result = declares_dev_dep && release_path_avoids_dev && dev_path_reuses_helper_flags && dev_tests_use_dev_helpers;

    println!("{}", result as i32);
}
