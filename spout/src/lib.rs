#![no_std]

extern crate alloc;

mod impls;
mod traits;

#[cfg(test)]
mod tests;

pub use impls::*;
pub use traits::{Flush, Spout};

#[cfg(feature = "std")]
pub use impls::ChannelSpout;
