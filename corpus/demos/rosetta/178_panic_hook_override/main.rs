fn main() {
    std::panic::set_hook(Box::new(|_| {
        println!("1");
    }));

    panic!("trigger");
}
