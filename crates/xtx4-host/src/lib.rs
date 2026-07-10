#![cfg_attr(not(target_arch = "x86_64"), no_std)]

#[cfg(target_arch = "riscv32")]
pub use xtx4_host_esp::{now_ms, delay_ms, Host};

#[cfg(target_arch = "x86_64")]
pub use xtx4_host_emulated::{now_ms, delay_ms, Host};
