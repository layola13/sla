fn main() {
    let iface_layout = include_str!("layout/slot.sal");
    let target = include_str!("bridge/link_target.sa");
    let consumer = include_str!("consumer/broken_consumer.sa");

    let target_takes_i32 = target.contains("@export link_target(slot: i32) -> i32");
    let consumer_allocates_slot_ptr = iface_layout.contains("#def Slot_SIZE = 4")
        && consumer.contains("slot = alloc Slot_SIZE")
        && consumer.contains("store slot+Slot_value, 1 as i32");
    let mismatch_is_explicit = consumer.contains("Intentional mismatch")
        && consumer.contains("call @link_target(&slot)");
    let signature_mismatch_fixture = target_takes_i32 && consumer_allocates_slot_ptr && mismatch_is_explicit;

    println!("{}", signature_mismatch_fixture as i32);
}
