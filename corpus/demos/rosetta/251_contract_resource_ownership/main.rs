fn main() {
    let iface = include_str!("iface/ownership.sai");
    let bridge = include_str!("bridge/ownership_bridge.sa");
    let consumer = include_str!("consumer/ownership_consumer.sa");

    let iface_declares_ownership_transfer = iface.contains("@extern take_ownership(handle: ptr) -> i32");
    let bridge_mutates_owned_handle = bridge.contains("@export take_ownership(handle: ptr) -> i32")
        && bridge.contains("load handle+0 as i32")
        && bridge.contains("store handle+0, next as i32");
    let consumer_passes_handle = consumer.contains("store handle+0, 251 as i32")
        && consumer.contains("call @take_ownership(handle)");
    let ownership_contract = iface_declares_ownership_transfer && bridge_mutates_owned_handle && consumer_passes_handle;

    println!("{}", ownership_contract as i32);
}
