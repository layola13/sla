use std::fs::File;
use std::os::fd::AsRawFd;

fn main() {
    let file = File::open("/dev/zero").unwrap();
    let fd = file.as_raw_fd();
    let mapped = fd + 1;
    println!("{}", mapped);
}
