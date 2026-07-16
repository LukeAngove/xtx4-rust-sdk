#![cfg(target_arch = "riscv32")]
#![no_std]

// Shared SPI bus for the Xteink X4.
//
// The ESP32-C3 has one usable SPI peripheral shared between the display
// (SSD1677, CS=GPIO21) and the SD card (CS=GPIO12). This crate provides
// a global singleton initialized once at boot.

use core::cell::RefCell;
use core::mem::MaybeUninit;
use critical_section::Mutex;
use esp_hal::spi::master::Spi;

type Inner = Spi<'static, esp_hal::Blocking>;
pub type SharedBus = Mutex<RefCell<Inner>>;

static BUS: Mutex<RefCell<MaybeUninit<Inner>>> =
    Mutex::new(RefCell::new(MaybeUninit::uninit()));

/// Initialize the shared SPI bus. Call once at boot, before constructing
/// any display or SD card drivers.
pub fn init(spi: Inner) {
    critical_section::with(|cs| {
        BUS.borrow_ref_mut(cs).write(spi);
    });
}

/// Get a `&'static` reference to the initialized bus for use with
/// `CriticalSectionDevice::new()` or direct access.
///
/// # Safety
///
/// This cast is sound because:
/// - `MaybeUninit<T>` is `#[repr(transparent)]` over `T`
/// - `RefCell` and `Mutex` are also `#[repr(transparent)]`
/// - The `Spi` is written once via `init()` and never moved or dropped
///
/// Must only be called after `init()`.
pub fn get() -> &'static SharedBus {
    // SAFETY: layout is identical (see above). Caller must ensure init() was called.
    unsafe { &*(core::ptr::from_ref(&BUS) as *const SharedBus) }
}

/// Run a closure with exclusive access to the bus.
pub fn with<R>(f: impl FnOnce(&mut Inner) -> R) -> R {
    critical_section::with(|cs| {
        let mut borrowed = BUS.borrow_ref_mut(cs);
        let spi = unsafe { borrowed.assume_init_mut() };
        f(spi)
    })
}
