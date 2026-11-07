fn main() {
    let iface = include_str!("iface/generic.sai");
    let implementation = include_str!("impl/generic_impl.sa");
    let consumer = include_str!("consumer/generic_consumer.sa");

    let iface_declares_shared_value = iface.contains("@extern shared_value(seed: i32) -> i32");
    let impl_exports_same_symbol = implementation.contains("@export shared_value(seed: i32) -> i32")
        && implementation.contains("value = add seed, 0");
    let consumer_imports_impl_once = consumer.contains("impl/generic_impl.sa")
        && consumer.contains("call @shared_value(245)");
    let shared_contract = iface_declares_shared_value && impl_exports_same_symbol && consumer_imports_impl_once;

    println!("{}", shared_contract as i32);
}
