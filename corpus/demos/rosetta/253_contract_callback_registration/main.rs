fn main() {
    let bridge = include_str!("bridge/callback_vtable.sa");
    let consumer = include_str!("consumer/callback_consumer.sa");

    let callback_vtable_layout = bridge.contains("@const CALLBACK_VT = vtable { call = @callback }")
        && bridge.contains("#def Slot_DATA = +0")
        && bridge.contains("#def Slot_VTABLE = +8")
        && bridge.contains("#def VTable_call = +0");
    let register_uses_indirect_call = bridge.contains("@register(&slot: ptr) -> i32")
        && bridge.contains("load vt+VTable_call as ptr")
        && bridge.contains("call_indirect fn(5)");
    let consumer_registers_vtable = consumer.contains("store slot+Slot_VTABLE, &CALLBACK_VT as ptr")
        && consumer.contains("call @register(&slot)");
    let callback_contract = callback_vtable_layout && register_uses_indirect_call && consumer_registers_vtable;

    println!("{}", callback_contract as i32);
}
