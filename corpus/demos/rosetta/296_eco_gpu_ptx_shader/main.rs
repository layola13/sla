fn main() {
    let shader = include_str!("guest/shader.sa");
    let layout = include_str!("guest/shader.sal");
    let args = include_str!("assets/kernel-args.txt");
    let launch = include_str!("host/launch/launch.json");
    let ptx = include_str!("host/ptx/kernel.ptx.note");

    let shader_exports_entry = shader.contains("@ffi_wrapper gpu_shader_gate(*uniform: ptr) -> i32")
        && shader.contains("@export gpu_shader_entry() -> i32");
    let layout_defines_uniform = layout.contains("#def ShaderUniform_lane = +0")
        && layout.contains("#def ShaderUniform_seed = +4");
    let host_launch_metadata = args.contains("lane=296")
        && launch.contains("\"gpu\": \"ptx\"")
        && launch.contains("\"entry\": \"gpu_shader_entry\"")
        && ptx.contains("PTX kernel text");
    let gpu_contract = shader_exports_entry && layout_defines_uniform && host_launch_metadata;

    println!("{}", gpu_contract as i32);
}
