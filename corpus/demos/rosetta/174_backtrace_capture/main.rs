fn synthetic_backtrace_depth(frames: [i32; 4]) -> i32 {
    let mut depth = 0;
    for frame in frames {
        if frame != 0 {
            depth += 1;
        }
    }
    depth
}

fn main() {
    let frames = [101, 202, 303, 0];
    let depth = synthetic_backtrace_depth(frames);
    println!("{}", depth);
}
