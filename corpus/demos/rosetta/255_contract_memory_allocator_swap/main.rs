fn main() {
    let iface = include_str!("iface/allocator.sai");
    let bridge = include_str!("bridge/allocator_bridge.sa");
    let consumer = include_str!("consumer/allocator_consumer.sa");

    let iface_declares_allocator_swap = iface.contains("@extern allocator_swap(handle: ptr) -> i32");
    let bridge_mutates_handle = bridge.contains("@export allocator_swap(handle: ptr) -> i32")
        && bridge.contains("load handle+0 as i32")
        && bridge.contains("store handle+0, next as i32");
    let consumer_selects_bridge = consumer.contains("bridge/allocator_bridge.sa")
        && consumer.contains("store handle+0, 255 as i32")
        && consumer.contains("call @allocator_swap(handle)");
    let allocator_contract = iface_declares_allocator_swap && bridge_mutates_handle && consumer_selects_bridge;

    println!("{}", allocator_contract as i32);
}
