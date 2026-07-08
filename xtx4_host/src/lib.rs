#![cfg_attr(not(target_arch = "x86_64"), no_std)]

#[cfg(target_arch = "riscv32")]
#[path = "host_esp.rs"]
mod host_impl;

#[cfg(target_arch = "x86_64")]
#[path = "host_emulated.rs"]
mod host_impl;

pub use host_impl::{now_ms, delay_ms, Host};