use std::fs::File;
use std::os::fd::AsRawFd;

fn main() {
    let file = File::open("/dev/null").unwrap();
    let fd = file.as_raw_fd();
    println!("{}", fd);
}
