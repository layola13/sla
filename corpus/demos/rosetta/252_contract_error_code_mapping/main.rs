fn main() {
    let iface = include_str!("iface/error_codes.sai");
    let bridge = include_str!("bridge/error_map.sa");
    let consumer = include_str!("consumer/error_consumer.sa");

    let iface_declares_mapping = iface.contains("@extern map_error(code: i32) -> i32");
    let bridge_branches_on_code = bridge.contains("@export map_error(code: i32) -> i32")
        && bridge.contains("ok = eq code, 252")
        && bridge.contains("L_MATCH")
        && bridge.contains("L_MISS");
    let consumer_uses_mapping = consumer.contains("bridge/error_map.sa")
        && consumer.contains("call @map_error(252)");
    let error_contract = iface_declares_mapping && bridge_branches_on_code && consumer_uses_mapping;

    println!("{}", error_contract as i32);
}
