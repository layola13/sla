fn main() {
    let config = include_str!("build/sysroot.toml");
    let stdio = include_str!("include/sysroot/stdio.h");
    let stdlib = include_str!("include/sysroot/stdlib.h");
    let generated = include_str!("generated/sysroot/layout.sa");

    let config_points_to_sysroot = config.contains("target = \"freestanding\"")
        && config.contains("include = \"include/sysroot\"");
    let headers_define_custom_surface = stdio.contains("int sa_puts(const char *s);")
        && stdlib.contains("void *sa_alloc(unsigned long size);");
    let generated_mentions_sysroot = generated.contains("build/sysroot.toml")
        && generated.contains("include/sysroot/*.h")
        && generated.contains("#def SYSROOT_LAYOUT_COUNT = 2");
    let sysroot_contract = config_points_to_sysroot && headers_define_custom_surface && generated_mentions_sysroot;

    println!("{}", sysroot_contract as i32);
}
