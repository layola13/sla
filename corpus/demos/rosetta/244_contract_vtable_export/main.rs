fn main() {
    let bridge = include_str!("bridge/button_vtable.sa");
    let consumer = include_str!("consumer/vtable_consumer.sa");

    let vtable_layout = bridge.contains("#def DynDraw_DATA = +0")
        && bridge.contains("#def DynDraw_VTABLE = +8")
        && bridge.contains("#def VTable_draw = +0");
    let exported_vtable = bridge.contains("@export button_draw(&self: ptr) -> i32")
        && bridge.contains("@const BUTTON_VT = vtable { draw = @button_draw }");
    let indirect_consumer = consumer.contains("load item+DynDraw_DATA as ptr")
        && consumer.contains("load item+DynDraw_VTABLE as ptr")
        && consumer.contains("call_indirect draw_fn(&data_ptr)")
        && consumer.contains("store fat+DynDraw_VTABLE, &BUTTON_VT as ptr");
    let vtable_contract = vtable_layout && exported_vtable && indirect_consumer;

    println!("{}", vtable_contract as i32);
}
