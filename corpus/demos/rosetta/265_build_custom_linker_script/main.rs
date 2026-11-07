fn main() {
    let config = include_str!("build/linker.toml");
    let script = include_str!("linker/linker.ld");
    let memory = include_str!("linker/memory.x");
    let generated = include_str!("generated/link_plan.sa");

    let config_wires_linker_inputs = config.contains("script = \"linker/linker.ld\"")
        && config.contains("memory = \"linker/memory.x\"")
        && config.contains("output = \"generated/link_plan.sa\"");
    let linker_files_are_real = script.contains("ENTRY(_start)")
        && script.contains(".text : { *(.text*) }")
        && memory.contains("FLASH : ORIGIN = 0x00000000, LENGTH = 64K")
        && memory.contains("RAM   : ORIGIN = 0x20000000, LENGTH = 16K");
    let generated_mentions_linker_sources = generated.contains("build/linker.toml")
        && generated.contains("linker/*.x, linker/*.ld")
        && generated.contains("@link_plan_value() -> i32");
    let linker_contract = config_wires_linker_inputs && linker_files_are_real && generated_mentions_linker_sources;

    println!("{}", linker_contract as i32);
}
