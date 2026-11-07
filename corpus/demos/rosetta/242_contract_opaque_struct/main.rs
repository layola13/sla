fn main() {
    let public_layout = include_str!("layout/public.sal");
    let private_layout = include_str!("layout/private.sal");
    let bridge = include_str!("bridge/opaque_bridge.sa");
    let consumer = include_str!("consumer/opaque_consumer.sa");

    let public_is_opaque = public_layout.contains("#def Opaque_SIZE = 16")
        && !public_layout.contains("Opaque_payload")
        && !public_layout.contains("Opaque_tag");
    let private_has_hidden_fields = private_layout.contains("#def Opaque_tag = +0")
        && private_layout.contains("#def Opaque_payload = +8");
    let bridge_can_touch_private_payload = bridge.contains("layout/private.sal")
        && bridge.contains("store slot+Opaque_payload, 242")
        && bridge.contains("load slot+Opaque_payload");
    let consumer_only_imports_public_layout = consumer.contains("layout/public.sal")
        && consumer.contains("bridge/opaque_bridge.sa")
        && !consumer.contains("Opaque_payload");
    let opaque_contract = public_is_opaque && private_has_hidden_fields && bridge_can_touch_private_payload && consumer_only_imports_public_layout;

    println!("{}", opaque_contract as i32);
}
