fn main() {
    let bridge = include_str!("bridge/callback_thunk.sa");
    let layout = include_str!("bridge/callback_thunk.sal");
    let ffi = include_str!("ffi/callback.sai");
    let registry = include_str!("host/callback/registry.h");
    let loader = include_str!("host/callback/loader.note");
    let thunk = include_str!("host/callback/thunk.md");

    let bridge_defines_callback_vtable = bridge.contains("@const CALLBACK_VT = vtable { call = @callback_step }")
        && bridge.contains("call_indirect fn(value)")
        && bridge.contains("store view+CallbackState_value, result as i32");
    let layout_defines_state_and_vtable = layout.contains("#def CallbackState_vtable = +8")
        && layout.contains("#def CallbackVTable_call = +0");
    let host_declares_registration = ffi.contains("@extern host_register_callback(cb: ptr) -> i32")
        && registry.contains("typedef int (*Demo290Callback)(int);")
        && registry.contains("int host_register_callback(Demo290Callback cb);");
    let host_documents_thunk = loader.contains("loader-side registration path")
        && thunk.contains("thunk pointer")
        && thunk.contains("callback registry");
    let callback_contract = bridge_defines_callback_vtable && layout_defines_state_and_vtable && host_declares_registration && host_documents_thunk;

    println!("{}", callback_contract as i32);
}
