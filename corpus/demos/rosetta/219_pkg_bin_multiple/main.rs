fn main() {
    let pkg = include_str!("sa.pkg");
    let bins = include_str!("bin/index.sa");
    let alpha = include_str!("bin/alpha/index.sa");
    let beta = include_str!("bin/beta/index.sa");
    let alpha_defs = include_str!("bin/alpha/helpers/alpha.sal");
    let beta_defs = include_str!("bin/beta/helpers/beta.sal");

    let package_named = pkg.contains("name = \"demo-219\"");
    let declares_both_bins = pkg.contains("bin/alpha") && pkg.contains("bin/beta");
    let aggregate_imports_both = bins.contains("bin/alpha/index.sa") && bins.contains("bin/beta/index.sa");
    let bin_entries_exist = alpha.contains("@alpha_bin_value()") && beta.contains("@beta_bin_value()");
    let bin_values_defined = alpha_defs.contains("ALPHA_BIN_VALUE = 100") && beta_defs.contains("BETA_BIN_VALUE = 119");
    let multiple_bins = package_named && declares_both_bins && aggregate_imports_both && bin_entries_exist && bin_values_defined;

    println!("{}", multiple_bins as i32);
}
