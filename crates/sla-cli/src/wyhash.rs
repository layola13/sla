//! Wyhash — a transliteration of Zig 0.17 `std.hash.Wyhash`
//! (lib/std/hash/wyhash.zig). Two call sites depend on its exact
//! values: the managed SAB cache path embeds `Wyhash.hash(0, file)`
//! (plugin_sab_paths.zig `managedSabPath`), and the freshness gate
//! keys/manifests are all Wyhash digests (plugin_freshness.zig,
//! cache_version.zig). One-shot `hash` is ported line for line; the
//! streaming form buffers the input and hashes it at `final` —
//! equivalent to Zig's incremental API, whose own std tests
//! (`verify.iterativeApi`, "iterative maintains last sixteen")
//! assert streaming == one-shot for every length. Test vectors
//! below are the wyhash reference vectors carried in the Zig std
//! source itself.

const SECRET: [u64; 4] = [
    0xa0761d6478bd642f,
    0xe7037ed1a0b428db,
    0x8ebc6af09c88c6e3,
    0x589965cc75374cc3,
];

fn read(bytes: &[u8]) -> u64 {
    let mut buf = [0u8; 8];
    buf[..bytes.len()].copy_from_slice(bytes);
    u64::from_le_bytes(buf)
}

fn mum(a: &mut u64, b: &mut u64) {
    let x = (*a as u128).wrapping_mul(*b as u128);
    *a = x as u64;
    *b = (x >> 64) as u64;
}

fn mix(a_: u64, b_: u64) -> u64 {
    let (mut a, mut b) = (a_, b_);
    mum(&mut a, &mut b);
    a ^ b
}

struct Core {
    a: u64,
    b: u64,
    state: [u64; 3],
    total_len: usize,
}

impl Core {
    fn new(seed: u64) -> Core {
        let s = seed ^ mix(seed ^ SECRET[0], SECRET[1]);
        Core {
            a: 0,
            b: 0,
            state: [s, s, s],
            total_len: 0,
        }
    }

    fn small_key(&mut self, input: &[u8]) {
        debug_assert!(input.len() <= 16);
        if input.len() >= 4 {
            let end = input.len() - 4;
            let quarter = (input.len() >> 3) << 2;
            self.a = (read(&input[0..4]) << 32) | read(&input[quarter..quarter + 4]);
            self.b = (read(&input[end..end + 4]) << 32)
                | read(&input[end - quarter..end - quarter + 4]);
        } else if !input.is_empty() {
            self.a = ((input[0] as u64) << 16)
                | ((input[input.len() >> 1] as u64) << 8)
                | input[input.len() - 1] as u64;
            self.b = 0;
        } else {
            self.a = 0;
            self.b = 0;
        }
    }

    fn round(&mut self, input: &[u8]) {
        debug_assert_eq!(input.len(), 48);
        for i in 0..3 {
            let a = read(&input[16 * i..16 * i + 8]);
            let b = read(&input[16 * i + 8..16 * i + 16]);
            self.state[i] = mix(a ^ SECRET[i + 1], b ^ self.state[i]);
        }
    }

    fn final0(&mut self) {
        self.state[0] ^= self.state[1] ^ self.state[2];
    }

    /// Zig `final1(input_lb, start_pos)`: `input_lb[start_pos..]`
    /// must be 1..=48 bytes and `input_lb` at least 16 bytes.
    fn final1(&mut self, input_lb: &[u8], start_pos: usize) {
        let input = &input_lb[start_pos..];
        let mut i = 0;
        while i + 16 < input.len() {
            self.state[0] = mix(
                read(&input[i..i + 8]) ^ SECRET[1],
                read(&input[i + 8..i + 16]) ^ self.state[0],
            );
            i += 16;
        }
        self.a = read(&input_lb[input_lb.len() - 16..input_lb.len() - 8]);
        self.b = read(&input_lb[input_lb.len() - 8..]);
    }

    fn final2(&mut self) -> u64 {
        self.a ^= SECRET[1];
        self.b ^= self.state[0];
        mum(&mut self.a, &mut self.b);
        mix(self.a ^ SECRET[0] ^ self.total_len as u64, self.b ^ SECRET[1])
    }
}

/// Zig `Wyhash.hash(seed, input)`.
pub fn wyhash(seed: u64, input: &[u8]) -> u64 {
    let mut h = Core::new(seed);
    if input.len() <= 16 {
        h.small_key(input);
    } else {
        let mut i = 0;
        if input.len() >= 48 {
            while i + 48 < input.len() {
                h.round(&input[i..i + 48]);
                i += 48;
            }
            h.final0();
        }
        h.final1(input, i);
    }
    h.total_len = input.len();
    h.final2()
}

/// Streaming form (Zig `Wyhash.init/update/final`); see the module
/// note for why buffering is a faithful mirror.
pub struct WyhashStream {
    seed: u64,
    buf: Vec<u8>,
}

impl WyhashStream {
    pub fn init(seed: u64) -> WyhashStream {
        WyhashStream {
            seed,
            buf: Vec::new(),
        }
    }

    pub fn update(&mut self, input: &[u8]) {
        self.buf.extend_from_slice(input);
    }

    pub fn finish(&self) -> u64 {
        wyhash(self.seed, &self.buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_vectors_from_zig_std() {
        // (seed, expected, input) — wyhash test_vector.cpp values as
        // carried in Zig's std/hash/wyhash.zig.
        let vectors: [(u64, u64, &[u8]); 7] = [
            (0, 0x0409638ee2bde459, b""),
            (1, 0xa8412d091b5fe0a9, b"a"),
            (2, 0x32dd92e4b2915153, b"abc"),
            (3, 0x8619124089a3a16b, b"message digest"),
            (4, 0x7a43afb61d7f5f40, b"abcdefghijklmnopqrstuvwxyz"),
            (
                5,
                0xff42329b90e50d58,
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789",
            ),
            (
                6,
                0xc39cab13b115aad3,
                b"12345678901234567890123456789012345678901234567890123456789012345678901234567890",
            ),
        ];
        for (seed, expected, input) in vectors {
            assert_eq!(wyhash(seed, input), expected, "input {input:?}");
        }
    }

    #[test]
    fn streaming_matches_one_shot_across_lengths_and_splits() {
        let base: Vec<u8> = (0..200u32).map(|i| (i * 37 + 11) as u8).collect();
        for len in 0..=130usize {
            let input = &base[..len];
            let want = wyhash(0, input);
            for split in [0usize, 1, 7, 16, 47, 48, 49, len] {
                if split > len {
                    continue;
                }
                let mut s = WyhashStream::init(0);
                s.update(&input[..split]);
                s.update(&input[split..]);
                assert_eq!(s.finish(), want, "len {len} split {split}");
            }
        }
    }
}
