fn main() {
    let iface = include_str!("iface/log.sai");
    let bridge = include_str!("bridge/log_bridge.sa");
    let consumer = include_str!("consumer/log_consumer.sa");

    let iface_declares_log_emit = iface.contains("@extern log_emit(level: i32, msg: ptr, len: u64) -> i32");
    let bridge_keeps_message_signature = bridge.contains("@export log_emit(level: i32, msg: ptr, len: u64) -> i32")
        && bridge.contains("ok = eq level, 257");
    let consumer_passes_message = consumer.contains("@const LOG_MESSAGE = utf8:\"257\\n\"")
        && consumer.contains("call @log_emit(257, &LOG_MESSAGE, 4)");
    let log_contract = iface_declares_log_emit && bridge_keeps_message_signature && consumer_passes_message;

    println!("{}", log_contract as i32);
}
