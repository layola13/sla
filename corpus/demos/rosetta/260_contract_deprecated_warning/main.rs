fn main() {
    let iface = include_str!("iface/deprecated.sai");
    let bridge = include_str!("bridge/deprecated_bridge.sa");
    let consumer = include_str!("consumer/deprecated_consumer.sa");

    let iface_exposes_legacy_symbol = iface.contains("@extern legacy_value() -> i32");
    let bridge_marks_deprecation = bridge.contains("@const DEPRECATED_NOTE = utf8:\"legacy\"")
        && bridge.contains("@export legacy_value() -> i32");
    let consumer_still_calls_legacy = consumer.contains("bridge/deprecated_bridge.sa")
        && consumer.contains("call @legacy_value()");
    let deprecated_contract = iface_exposes_legacy_symbol && bridge_marks_deprecation && consumer_still_calls_legacy;

    println!("{}", deprecated_contract as i32);
}
