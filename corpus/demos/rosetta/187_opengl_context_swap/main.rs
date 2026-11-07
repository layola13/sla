extern "C" {
    fn gl_make_current(ctx: *mut core::ffi::c_void) -> i32;
    fn gl_swap_buffers(ctx: *mut core::ffi::c_void) -> i32;
}

fn main() {
    unsafe {
        let ctx = core::ptr::null_mut();
        let _ = gl_make_current(ctx);
        let _ = gl_swap_buffers(ctx);
    }
    println!("{}", 1);
}
