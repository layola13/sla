fn main() {
    let layout = include_str!("layout/tls.sal");
    let bridge = include_str!("bridge/tls_bridge.sa");
    let consumer = include_str!("consumer/tls_consumer.sa");

    let tls_layout = layout.contains("#def TLS_SIZE = 8")
        && layout.contains("#def TLS_value = +0");
    let bridge_updates_slot = bridge.contains("@export tls_write(&slot: ptr) -> i32")
        && bridge.contains("load slot+0 as i32")
        && bridge.contains("store slot+0, next as i32");
    let consumer_allocates_tls = consumer.contains("layout/tls.sal")
        && consumer.contains("tls = alloc TLS_SIZE")
        && consumer.contains("call @tls_write(&tls)");
    let tls_contract = tls_layout && bridge_updates_slot && consumer_allocates_tls;

    println!("{}", tls_contract as i32);
}
