fn main() {
    let host = include_str!("host/plugin_host.sa");
    let implementation = include_str!("impl/plugin_impl.sa");
    let consumer = include_str!("consumer/plugin_consumer.sa");

    let host_declares_dispatch = host.contains("@extern plugin_dispatch(tag: i32) -> i32");
    let impl_exports_dispatch = implementation.contains("@export plugin_dispatch(tag: i32) -> i32")
        && implementation.contains("return 254");
    let consumer_loads_plugin_impl = consumer.contains("impl/plugin_impl.sa")
        && consumer.contains("call @plugin_dispatch(254)");
    let plugin_contract = host_declares_dispatch && impl_exports_dispatch && consumer_loads_plugin_impl;

    println!("{}", plugin_contract as i32);
}
