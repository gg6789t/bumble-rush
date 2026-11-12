//! Tiny dependency-free RNG (xorshift64*) plus a stateless hash for scenery.

pub struct Rng(u64);

impl Rng {
    pub fn seeded() -> Self {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x1234_5678);
        Rng((t ^ 0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in [0, 1).
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / 16_777_216.0
    }

    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        if hi <= lo {
            lo
        } else {
            lo + (hi - lo) * self.f32()
        }
    }

    pub fn chance(&mut self, p: f32) -> bool {
        self.f32() < p
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n.max(1) as u64) as usize
    }
}

/// Deterministic hash used to place decorations (flowers, trees, bricks)
/// so they stay put as the world scrolls.
pub fn hash(n: i64) -> u32 {
    let mut x = (n as u64) ^ 0x9E37_79B9_7F4A_7C15;
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (x ^ (x >> 31)) as u32
}
