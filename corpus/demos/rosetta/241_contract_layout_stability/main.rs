fn main() {
    let layout = include_str!("layout/point.sal");
    let bridge = include_str!("bridge/point_bridge.sa");
    let consumer = include_str!("consumer/point_consumer.sa");

    let stable_offsets = layout.contains("#def Point_SIZE = 16")
        && layout.contains("#def Point_x = +0")
        && layout.contains("#def Point_y = +8");
    let bridge_uses_same_offsets = bridge.contains("BridgePoint_x = +0")
        && bridge.contains("BridgePoint_y = +8")
        && bridge.contains("load point+BridgePoint_x")
        && bridge.contains("load point+BridgePoint_y");
    let consumer_exercises_layout = consumer.contains("layout/point.sal")
        && consumer.contains("bridge/point_bridge.sa")
        && consumer.contains("store point+Point_x, 24")
        && consumer.contains("store point+Point_y, 217");
    let contract_layout = stable_offsets && bridge_uses_same_offsets && consumer_exercises_layout;

    println!("{}", contract_layout as i32);
}
