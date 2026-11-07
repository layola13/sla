fn main() {
    let entry = include_str!("cycle/index.sa");
    let core = include_str!("cycle/core/index.sa");
    let a = include_str!("cycle/core/a.sa");
    let b = include_str!("cycle/core/b.sa");
    let alpha = include_str!("cycle/alpha/index.sa");
    let beta = include_str!("cycle/beta/index.sa");
    let gamma = include_str!("cycle/gamma/index.sa");

    let entry_reaches_core = entry.contains("cycle/core/index.sa") && core.contains("cycle/core/a.sa");
    let core_cycle = a.contains("cycle/core/b.sa") && b.contains("cycle/core/a.sa");
    let sibling_cycle = alpha.contains("../beta/index.sa") && beta.contains("../gamma/index.sa") && gamma.contains("../alpha/index.sa");
    let diagnostic_fixture = entry_reaches_core && core_cycle && sibling_cycle;

    println!("{}", diagnostic_fixture as i32);
}
