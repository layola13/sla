fn main() {
    let module = include_str!("kernel/module.sa");
    let layout = include_str!("kernel/module.sal");
    let manifest = include_str!("host/module/kernel-module.manifest");
    let insmod = include_str!("host/module/insmod.txt");
    let linker = include_str!("linker/kernel-module.ld");

    let kernel_exports_entry = module.contains("@ffi_wrapper kernel_gate(*record: ptr) -> i32")
        && module.contains("@export kernel_module_entry() -> i32");
    let layout_defines_record = layout.contains("#def KernelRecord_major = +0")
        && layout.contains("#def KernelRecord_minor = +4");
    let host_kernel_metadata = manifest.contains("module = \"demo-294\"")
        && manifest.contains("type = \"kernel\"")
        && insmod.contains("load the kernel module artifact")
        && linker.contains(".text : { *(.text*) }");
    let kernel_contract = kernel_exports_entry && layout_defines_record && host_kernel_metadata;

    println!("{}", kernel_contract as i32);
}
