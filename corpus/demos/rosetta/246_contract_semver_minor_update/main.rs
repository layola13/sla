fn main() {
    let iface = include_str!("iface/minor.sai");
    let implementation = include_str!("impl/minor_impl.sa");
    let consumer = include_str!("consumer/minor_consumer.sa");

    let iface_keeps_original = iface.contains("@extern minor_value() -> i32");
    let iface_adds_extension = iface.contains("@extern minor_extension() -> i32");
    let impl_exports_both = implementation.contains("@export minor_value() -> i32")
        && implementation.contains("@export minor_extension() -> i32");
    let consumer_accepts_added_api = consumer.contains("call @minor_value()")
        && consumer.contains("call @minor_extension()");
    let semver_minor_contract = iface_keeps_original && iface_adds_extension && impl_exports_both && consumer_accepts_added_api;

    println!("{}", semver_minor_contract as i32);
}
