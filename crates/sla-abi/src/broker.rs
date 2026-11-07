//! Host broker cache structs (`plugin_api.BrokerCache*`, all
//! `extern struct` in the original). The plugin stores/retrieves
//! compile-cache artifacts through the host broker; when the broker is
//! unavailable (`Context::broker_available` is false) the original
//! takes a no-cache path and so does this port.

/// `plugin_api.broker_op_cache_lookup`.
pub const BROKER_OP_CACHE_LOOKUP: u32 = 5;
/// `plugin_api.broker_op_cache_store`.
pub const BROKER_OP_CACHE_STORE: u32 = 6;

/// Lookup request (64 B). `value_ptr`/`value_cap` are the caller's
/// output buffer; the response reports the full value length.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BrokerCacheLookupRequest {
    pub ns_ptr: *const u8,
    pub ns_len: usize,
    pub key_ptr: *const u8,
    pub key_len: usize,
    pub closure_ptr: *const u8,
    pub closure_len: usize,
    pub value_ptr: *mut u8,
    pub value_cap: usize,
}

/// Lookup response (16 B): `hit` is 0/1, `value_len` the stored length.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BrokerCacheLookupResponse {
    pub hit: u32,
    pub value_len: usize,
}

/// Store request (64 B).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BrokerCacheStoreRequest {
    pub ns_ptr: *const u8,
    pub ns_len: usize,
    pub key_ptr: *const u8,
    pub key_len: usize,
    pub closure_ptr: *const u8,
    pub closure_len: usize,
    pub value_ptr: *const u8,
    pub value_len: usize,
}

/// Store response (4 B): `stored` is 0/1.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BrokerCacheStoreResponse {
    pub stored: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    // Values measured on the reference implementation:
    // docs/probes/abi_probe.out.txt.
    #[test]
    fn broker_layouts_match_reference() {
        assert_eq!(size_of::<BrokerCacheLookupRequest>(), 64);
        assert_eq!(size_of::<BrokerCacheLookupResponse>(), 16);
        assert_eq!(size_of::<BrokerCacheStoreRequest>(), 64);
        assert_eq!(size_of::<BrokerCacheStoreResponse>(), 4);
    }
}
