fn main() {
    let layout = include_str!("layout/init_order.sal");
    let bridge = include_str!("bridge/init_bridge.sa");
    let consumer = include_str!("consumer/init_consumer.sa");

    let init_layout = layout.contains("#def Init_first = +0")
        && layout.contains("#def Init_second = +8");
    let bridge_exports_ordered_steps = bridge.contains("@export init_first() -> i32")
        && bridge.contains("@export init_second() -> i32");
    let consumer_stores_in_order = consumer.contains("first = call @init_first()")
        && consumer.contains("store state+Init_first, first as i32")
        && consumer.contains("second = call @init_second()")
        && consumer.contains("store state+Init_second, second as i32");
    let init_contract = init_layout && bridge_exports_ordered_steps && consumer_stores_in_order;

    println!("{}", init_contract as i32);
}
