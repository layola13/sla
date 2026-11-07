fn main() {
    let iface = include_str!("iface/consts.sai");
    let implementation = include_str!("impl/const_impl.sa");
    let consumer = include_str!("consumer/const_consumer.sa");

    let iface_declares_const_value = iface.contains("@extern const_value() -> i32");
    let impl_exports_text_const = implementation.contains("@const EXPORTED_TEXT = utf8:\"250\\n\"");
    let impl_exports_value = implementation.contains("@export const_value() -> i32");
    let consumer_reads_export = consumer.contains("impl/const_impl.sa")
        && consumer.contains("call @const_value()");
    let const_contract = iface_declares_const_value && impl_exports_text_const && impl_exports_value && consumer_reads_export;

    println!("{}", const_contract as i32);
}
