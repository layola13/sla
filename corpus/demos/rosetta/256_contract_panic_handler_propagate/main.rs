fn main() {
    let iface = include_str!("iface/panic.sai");
    let host = include_str!("host/panic_handler.sa");
    let consumer = include_str!("consumer/panic_consumer.sa");

    let iface_declares_hook = iface.contains("@extern panic_hook(code: i32) -> i32");
    let host_exports_hook = host.contains("@export panic_hook(code: i32) -> i32")
        && host.contains("ok = eq code, 256")
        && host.contains("L_OK")
        && host.contains("L_ERR");
    let consumer_routes_to_host = consumer.contains("host/panic_handler.sa")
        && consumer.contains("call @panic_hook(256)");
    let panic_contract = iface_declares_hook && host_exports_hook && consumer_routes_to_host;

    println!("{}", panic_contract as i32);
}
