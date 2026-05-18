//! Noise sources — pseudo-random sample generators for percussion,
//! noise oscillators, sample-and-hold modulation, etc.

mod xorshift;

pub use xorshift::NoiseSource;
