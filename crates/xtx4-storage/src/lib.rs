#![cfg_attr(target_arch = "riscv32", no_std)]

// Storage abstraction: SD card on ESP32, host folder on desktop.
//
// ESP32:   SD card over SPI on GPIO 12, shares the bus via xtx4_bus + CriticalSectionDevice
// Desktop: reads from ./sd_root/ directory

#[derive(Debug)]
pub enum Error {
    NotFound,
    ReadError,
    WriteError,
}

// ── ESP32 backend ───────────────────────────────────────────────────────

#[cfg(target_arch = "riscv32")]
mod sd_backend {
    use super::Error;
    use core::convert::Infallible;
    use embedded_hal::spi::{Operation, SpiBus, SpiDevice};
    use embedded_sdmmc::{Mode, SdCard, TimeSource, Timestamp, VolumeIdx, VolumeManager};
    use esp_hal::{
        delay::Delay,
        spi::master::Spi,
    };

    // ── GPIO12 CS via PAC registers ──────────────────────────────────
    // GPIO12 is not exposed by esp-hal on ESP32-C3 (normally reserved
    // for flash SPIHD), but the X4 uses DIO flash mode freeing it for
    // SD card chip select. We write the registers directly, matching
    // what Arduino's gpio_set_level(12, ...) does under the hood.

    fn cs_init() {
        // SAFETY: direct register write, no lock required for GPIO output control
        let gpio = esp_hal::peripherals::GPIO::regs();
        gpio.enable_w1ts().write(|w| unsafe { w.bits(1 << 12) });
        gpio.out_w1ts().write(|w| unsafe { w.bits(1 << 12) });
    }

    struct SdDev;

    impl embedded_hal::spi::ErrorType for SdDev {
        type Error = Infallible;
    }

    impl SpiDevice<u8> for SdDev {
        fn transaction(
            &mut self,
            operations: &mut [Operation<'_, u8>],
        ) -> Result<(), Self::Error> {
            xtx4_bus::with(|spi: &mut Spi<'static, esp_hal::Blocking>| {
                let gpio = esp_hal::peripherals::GPIO::regs();
                // assert CS
                gpio.out_w1tc().write(|w| unsafe { w.bits(1 << 12) });

                for op in operations {
                    match op {
                        Operation::Read(buf) => { spi.read(buf).unwrap(); }
                        Operation::Write(data) => { spi.write(data).unwrap(); }
                        Operation::Transfer(read, write) => { SpiBus::transfer(spi, read, write).unwrap(); }
                        Operation::TransferInPlace(buf) => { spi.transfer(buf).unwrap(); }
                        Operation::DelayNs(_) => {}
                    }
                }

                // deassert CS
                gpio.out_w1ts().write(|w| unsafe { w.bits(1 << 12) });
            });
            Ok(())
        }
    }

    // ── Path helpers ───────────────────────────────────────────────────
    // embedded-sdmmc's open_file_in_dir takes 8.3 filenames only.
    // We parse Unix-style paths by stripping leading '/' and splitting
    // directory components from the filename.

    fn open_path<'a>(
        root: &'a embedded_sdmmc::Directory<'a, Sd, DummyTimeSource, 4, 4, 1>,
        path: &str,
        mode: Mode,
    ) -> Result<embedded_sdmmc::File<'a, Sd, DummyTimeSource, 4, 4, 1>, ()> {
        let path = path.trim_start_matches('/');
        let dir_path = path.trim_end_matches(|c| c != '/');
        let filename = path[dir_path.len()..].trim_start_matches('/');

        if filename.is_empty() {
            return Err(());
        }

        // For now, only support files in root. TODO: walk subdirectories.
        let subdirs_part = dir_path.trim_end_matches('/');
        if !subdirs_part.is_empty() {
            return Err(());
        }

        root.open_file_in_dir(filename, mode).map_err(|_| ())
    }

    // ── Backend ───────────────────────────────────────────────────────

    struct DummyTimeSource;
    impl TimeSource for DummyTimeSource {
        fn get_timestamp(&self) -> Timestamp {
            Timestamp::from_calendar(2024, 1, 1, 0, 0, 0).unwrap()
        }
    }

    type Sd = SdCard<SdDev, Delay>;
    type Vm = VolumeManager<Sd, DummyTimeSource>;

    pub struct SdBackend {
        volume_mgr: Vm,
    }

    impl SdBackend {
        pub fn new() -> Self {
            cs_init();
            let sdcard = SdCard::new(SdDev, Delay::new());
            let volume_mgr = VolumeManager::new(sdcard, DummyTimeSource);
            Self { volume_mgr }
        }

        pub fn read_file(&mut self, path: &str, buf: &mut [u8]) -> Result<usize, Error> {
            let volume = self.volume_mgr
                .open_volume(VolumeIdx(0))
                .map_err(|e| map_err(e))?;
            let root = volume.open_root_dir().map_err(|e| map_err(e))?;
            let file = open_path(&root, path, Mode::ReadOnly)
                .map_err(|_| Error::NotFound)?;

            let mut total: usize = 0;
            while !file.is_eof() && total < buf.len() {
                let n = file.read(&mut buf[total..]).map_err(|e| map_err(e))?;
                total += n;
            }
            Ok(total)
        }

        pub fn write_file(&mut self, path: &str, data: &[u8]) -> Result<(), Error> {
            let volume = self.volume_mgr
                .open_volume(VolumeIdx(0))
                .map_err(|e| map_err(e))?;
            let root = volume.open_root_dir().map_err(|e| map_err(e))?;
            let file = open_path(&root, path, Mode::ReadWriteCreateOrTruncate)
                .map_err(|_| Error::WriteError)?;
            file.write(data).map_err(|e| map_err(e))?;
            file.close().map_err(|e| map_err(e))?;
            Ok(())
        }

        pub fn list_dir(
            &mut self,
            path: &str,
            f: &mut dyn FnMut(&str) -> bool,
        ) -> Result<(), Error> {
            let volume = self.volume_mgr
                .open_volume(VolumeIdx(0))
                .map_err(|e| map_err(e))?;
            let root = volume.open_root_dir().map_err(|e| map_err(e))?;

            let dir = if path == "/" || path.is_empty() {
                root
            } else {
                root.open_dir(path).map_err(|e| map_err(e))?
            };

            dir.iterate_dir(|entry| {
                let raw = entry.name.base_name();
                if let Ok(name) = core::str::from_utf8(raw) {
                    f(name);
                }
            })
            .map_err(|e| map_err(e))?;

            Ok(())
        }

        pub fn exists(&self, _path: &str) -> bool {
            if let Ok(volume) = self.volume_mgr.open_volume(VolumeIdx(0)) {
                if let Ok(root) = volume.open_root_dir() {
                    return root.open_file_in_dir(_path, Mode::ReadOnly).is_ok();
                }
            }
            false
        }
    }

    fn map_err<E: core::fmt::Debug>(e: embedded_sdmmc::Error<E>) -> Error {
        match e {
            embedded_sdmmc::Error::NotFound => Error::NotFound,
            _ => Error::ReadError,
        }
    }
}

// ── Host (desktop) backend ──────────────────────────────────────────────

#[cfg(target_arch = "x86_64")]
mod host_backend {
    use super::Error;
    use std::fs;
    use std::path::PathBuf;

    const ROOT: &str = "./sd_root";

    pub struct HostBackend {
        root: PathBuf,
    }

    impl HostBackend {
        pub fn new() -> Self {
            let root = PathBuf::from(ROOT);
            let _ = fs::create_dir_all(&root);
            Self { root }
        }

        pub fn read_file(&mut self, path: &str, buf: &mut [u8]) -> Result<usize, Error> {
            let full = self.root.join(path.trim_start_matches('/'));
            let data = fs::read(&full).map_err(|_| Error::NotFound)?;
            let len = data.len().min(buf.len());
            buf[..len].copy_from_slice(&data[..len]);
            Ok(len)
        }

        pub fn write_file(&mut self, path: &str, data: &[u8]) -> Result<(), Error> {
            let full = self.root.join(path.trim_start_matches('/'));
            if let Some(parent) = full.parent() {
                let _ = fs::create_dir_all(parent);
            }
            fs::write(&full, data).map_err(|_| Error::WriteError)
        }

        pub fn list_dir(
            &mut self,
            path: &str,
            f: &mut dyn FnMut(&str) -> bool,
        ) -> Result<(), Error> {
            let full = self.root.join(path.trim_start_matches('/'));
            let entries = fs::read_dir(&full).map_err(|_| Error::NotFound)?;
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if !f(name) {
                        break;
                    }
                }
            }
            Ok(())
        }

        pub fn exists(&self, path: &str) -> bool {
            self.root.join(path.trim_start_matches('/')).exists()
        }
    }
}

// ── Public Storage type ──────────────────────────────────────────────────

pub struct Storage {
    #[cfg(target_arch = "riscv32")]
    inner: sd_backend::SdBackend,
    #[cfg(target_arch = "x86_64")]
    inner: host_backend::HostBackend,
}

impl Storage {
    #[cfg(target_arch = "riscv32")]
    pub fn new() -> Self {
        Self { inner: sd_backend::SdBackend::new() }
    }

    #[cfg(target_arch = "x86_64")]
    pub fn new() -> Self {
        Self { inner: host_backend::HostBackend::new() }
    }

    pub fn read_file(&mut self, path: &str, buf: &mut [u8]) -> Result<usize, Error> {
        self.inner.read_file(path, buf)
    }

    pub fn write_file(&mut self, path: &str, data: &[u8]) -> Result<(), Error> {
        self.inner.write_file(path, data)
    }

    pub fn list_dir(
        &mut self,
        path: &str,
        f: &mut dyn FnMut(&str) -> bool,
    ) -> Result<(), Error> {
        self.inner.list_dir(path, f)
    }

    pub fn exists(&self, path: &str) -> bool {
        self.inner.exists(path)
    }
}

// ── Tests (desktop backend only) ───────────────────────────────────────

#[cfg(all(test, target_arch = "x86_64"))]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn write_and_read() {
        let mut s = Storage::new();
        let path = "__test_write_read.pbm";
        let data = b"P4\n4 4\n\x00\x00";

        s.write_file(path, data).unwrap();
        assert!(s.exists(path));

        let mut buf = [0u8; 128];
        let n = s.read_file(path, &mut buf).unwrap();
        assert_eq!(&buf[..n], data);

        let _ = fs::remove_file(format!("./sd_root/{}", path));
    }

    #[test]
    fn list_and_exists() {
        let mut s = Storage::new();
        s.write_file("__test_list_a.txt", b"hello").unwrap();
        s.write_file("__test_list_b.txt", b"world").unwrap();

        let mut found = Vec::new();
        s.list_dir("/", &mut |name| {
            if name.starts_with("__test_list") {
                found.push(name.to_string());
            }
            true
        })
        .unwrap();

        assert_eq!(found.len(), 2);
        assert!(s.exists("__test_list_a.txt"));
        assert!(!s.exists("__no_such_file.xyz"));

        let _ = fs::remove_file("./sd_root/__test_list_a.txt");
        let _ = fs::remove_file("./sd_root/__test_list_b.txt");
    }

    #[test]
    fn pbm_roundtrip() {
        let mut s = Storage::new();
        let pbm = b"P4\n8 8\n\xAA\x55\xAA\x55\xAA\x55\xAA\x55";
        s.write_file("__test_roundtrip.pbm", pbm).unwrap();

        let mut buf = [0u8; 128];
        let n = s.read_file("__test_roundtrip.pbm", &mut buf).unwrap();
        assert_eq!(&buf[..n], pbm);

        let _ = fs::remove_file("./sd_root/__test_roundtrip.pbm");
    }
}
