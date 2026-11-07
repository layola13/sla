fn main() {
    let v1 = include_str!("iface/v1.sai");
    let v2 = include_str!("iface/v2.sai");
    let implementation = include_str!("bridge/major_break_impl.sa");
    let consumer = include_str!("consumer/major_consumer.sa");

    let old_signature = v1.contains("@extern major_value(slot: i32) -> i32");
    let new_signature = v2.contains("@extern major_value(handle: ptr) -> i32");
    let impl_uses_new_signature = implementation.contains("@export major_value(handle: ptr) -> i32");
    let consumer_allocates_handle = consumer.contains("handle = alloc 8")
        && consumer.contains("call @major_value(handle)");
    let major_break_contract = old_signature && new_signature && impl_uses_new_signature && consumer_allocates_handle;

    println!("{}", major_break_contract as i32);
}
