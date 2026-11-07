fn main() {
    let macros = include_str!("macros/store.sa");
    let bridge = include_str!("bridge/macro_bridge.sa");
    let consumer = include_str!("consumer/macro_consumer.sa");

    let macro_is_exported_fixture = macros.contains("[MACRO] STORE_I32 %base, %value")
        && macros.contains("store %base+0, %value as i32")
        && macros.contains("[END_MACRO]");
    let bridge_exports_helper = bridge.contains("@export macro_value() -> i32");
    let consumer_expands_macro = consumer.contains("macros/store.sa")
        && consumer.contains("EXPAND STORE_I32 cell, 249")
        && consumer.contains("call @macro_value()");
    let macro_contract = macro_is_exported_fixture && bridge_exports_helper && consumer_expands_macro;

    println!("{}", macro_contract as i32);
}
