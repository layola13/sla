fn main() {
    let layout = include_str!("layout/slot.sal");
    let bridge = include_str!("bridge/boundary.sa");
    let consumer = include_str!("consumer/boundary_consumer.sa");

    let layout_defines_slot = layout.contains("#def Slot_SIZE = 4")
        && layout.contains("#def Slot_value = +0");
    let ffi_wrapper_marks_boundary = bridge.contains("@ffi_wrapper boundary(*raw: ptr) -> i32")
        && bridge.contains("assume_borrow raw")
        && bridge.contains("assume_safe raw")
        && bridge.contains("store safe+0, next as i32");
    let consumer_crosses_raw_edge = consumer.contains("bridge/boundary.sa")
        && consumer.contains("call @boundary(*slot)")
        && consumer.contains("store slot+Slot_value, 248");
    let boundary_contract = layout_defines_slot && ffi_wrapper_marks_boundary && consumer_crosses_raw_edge;

    println!("{}", boundary_contract as i32);
}
