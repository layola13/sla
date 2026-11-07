struct Data { val: i32 }

fn require_send<T: Send>(val: T) -> i32 {
    let _ = val;
    42
}

fn main() {
    let d = Data { val: 0 };
    let res = require_send(d);
    println!("{}", res);
}
