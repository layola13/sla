fn main() {
    let plan = include_str!("build/codegen-plan.txt");
    let config = include_str!("build/codegen.toml");
    let generated = include_str!("generated/codegen.sa");

    let plan_has_steps = plan.contains("read the manifest")
        && plan.contains("emit the generated SA-ASM module")
        && plan.contains("keep the output under generated/");
    let config_wires_input_output = config.contains("input = \"build/codegen-plan.txt\"")
        && config.contains("output = \"generated/codegen.sa\"");
    let generated_mentions_source = generated.contains("build/codegen.toml")
        && generated.contains("build/codegen-plan.txt")
        && generated.contains("@codegen_value() -> i32");
    let codegen_contract = plan_has_steps && config_wires_input_output && generated_mentions_source;

    println!("{}", codegen_contract as i32);
}
