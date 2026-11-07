fn main() {
    #[cfg(target_arch = "x86_64")]
    {
        let arch = "x86";
        println!("{arch}");
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        let arch = "fallback";
        println!("{arch}");
    }
}
