fn main() {
    let helper = include_str!("helper.sa");
    let step = include_str!("chain/step.sa");
    let seed = include_str!("chain/deeper/seed.sa");

    let helper_imports_step = helper.contains("@import \"chain/step.sa\"") && helper.contains("@helper_value()");
    let step_imports_seed = step.contains("@import \"chain/deeper/seed.sa\"") && step.contains("@export step_value()");
    let seed_exports_value = seed.contains("@export seed_value()");
    let relative_chain = helper_imports_step && step_imports_seed && seed_exports_value;

    println!("{}", relative_chain as i32);
}
