extern "C" {
    fn dlopen(path: *const u8, flags: i32) -> *mut core::ffi::c_void;
    fn dlsym(handle: *mut core::ffi::c_void, symbol: *const u8) -> *mut core::ffi::c_void;
    fn dlclose(handle: *mut core::ffi::c_void) -> i32;
}

fn main() {
    unsafe {
        let handle = dlopen(b"libdemo.so\0".as_ptr(), 2);
        let _symbol = dlsym(handle, b"demo_entry\0".as_ptr());
        let _ = dlclose(handle);
    }
    println!("{}", 1);
}
