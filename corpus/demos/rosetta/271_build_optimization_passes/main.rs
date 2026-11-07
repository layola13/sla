fn main() {
    let config = include_str!("build/optimizations/passes.toml");
    let order = include_str!("cache/optimizations/order.txt");
    let generated = include_str!("generated/optimizations/passes.sa");

    let config_lists_passes = config.contains("inline")
        && config.contains("const-prop")
        && config.contains("dce");
    let order_preserves_sequence = order.starts_with("inline\nconst-prop\ndce");
    let generated_counts_passes = generated.contains("build/optimizations/passes.toml")
        && generated.contains("cache/optimizations/order.txt")
        && generated.contains("#def OPTIMIZATION_PASS_COUNT = 3")
        && generated.contains("@optimization_passes_count() -> i32");
    let optimization_contract = config_lists_passes && order_preserves_sequence && generated_counts_passes;

    println!("{}", optimization_contract as i32);
}
