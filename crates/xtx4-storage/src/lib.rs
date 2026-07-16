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

#[derive(Debug, Clone, Copy)]
pub enum SeekFrom {
    Start(u32),
    Current(i32),
    End(u32),
}

/// File handle returned by [`Storage::open`].
pub trait File {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error>;
    fn write(&mut self, data: &[u8]) -> Result<usize, Error>;
    fn seek(&mut self, pos: SeekFrom) -> Result<usize, Error>;
    fn stream_position(&mut self) -> Result<usize, Error>;
    fn length(&self) -> Result<usize, Error>;
}

// ── ESP32 backend ───────────────────────────────────────────────────────

#[cfg(target_arch = "riscv32")]
mod sd_backend {
    use super::{Error, File, SeekFrom};
    use core::convert::Infallible;
    use embedded_hal::spi::{Operation, SpiBus, SpiDevice};
    use embedded_sdmmc::{Mode, RawDirectory, RawFile, RawVolume, SdCard, TimeSource, Timestamp, VolumeIdx, VolumeManager};
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

        pub fn list_dir(
            &self,
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

        pub fn exists(&self, path: &str) -> bool {
            let name = path.trim_start_matches('/');
            if let Ok(volume) = self.volume_mgr.open_volume(VolumeIdx(0)) {
                if let Ok(root) = volume.open_root_dir() {
                    return root.open_file_in_dir(name, Mode::ReadOnly).is_ok();
                }
            }
            false
        }

        pub fn open_file(&self, path: &str, mode: Mode) -> Result<SdFile, Error> {
            SdFile::open(&self.volume_mgr, path, mode)
        }
    }

    pub struct SdFile {
        raw_volume: RawVolume,
        raw_dir: RawDirectory,
        raw_file: RawFile,
        vm: &'static Vm,
    }

    impl SdFile {
        fn open(vm: &Vm, path: &str, mode: Mode) -> Result<Self, Error> {
            let raw_volume = vm.open_raw_volume(VolumeIdx(0)).map_err(|e| map_err(e))?;
            let raw_dir = vm.open_root_dir(raw_volume).map_err(|e| {
                vm.close_volume(raw_volume).ok();
                map_err(e)
            })?;
            let filename = path.trim_start_matches('/');
            if filename.contains('/') {
                vm.close_dir(raw_dir).ok();
                vm.close_volume(raw_volume).ok();
                return Err(Error::NotFound);
            }
            let raw_file = vm.open_file_in_dir(raw_dir, filename, mode).map_err(|e| {
                vm.close_dir(raw_dir).ok();
                vm.close_volume(raw_volume).ok();
                map_err(e)
            })?;
            // SAFETY: VolumeManager lives inside Storage, which lives inside
            // XtX4, which lives for the entire program (never dropped on ESP32).
            let vm: &'static Vm = unsafe { &*(vm as *const Vm) };
            Ok(SdFile { raw_volume, raw_dir, raw_file, vm })
        }
    }

    impl File for SdFile {
        fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
            self.vm.read(self.raw_file, buf).map_err(|e| map_err(e))
        }

        fn write(&mut self, data: &[u8]) -> Result<usize, Error> {
            self.vm.write(self.raw_file, data).map_err(|e| map_err(e))?;
            Ok(data.len())
        }

        fn seek(&mut self, pos: SeekFrom) -> Result<usize, Error> {
            match pos {
                SeekFrom::Start(o) => self.vm.file_seek_from_start(self.raw_file, o),
                SeekFrom::Current(o) => self.vm.file_seek_from_current(self.raw_file, o),
                SeekFrom::End(o) => self.vm.file_seek_from_end(self.raw_file, o),
            }
            .map_err(|e| map_err(e))?;
            self.stream_position()
        }

        fn stream_position(&mut self) -> Result<usize, Error> {
            self.vm.file_offset(self.raw_file).map(|o| o as usize).map_err(|e| map_err(e))
        }

        fn length(&self) -> Result<usize, Error> {
            self.vm.file_length(self.raw_file).map(|l| l as usize).map_err(|e| map_err(e))
        }
    }

    impl Drop for SdFile {
        fn drop(&mut self) {
            self.vm.close_file(self.raw_file).ok();
            self.vm.close_dir(self.raw_dir).ok();
            self.vm.close_volume(self.raw_volume).ok();
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
    use super::{Error, File, SeekFrom};
    use std::fs;
    use std::path::PathBuf;

    const ROOT: &str = "./sd_root";

    pub struct HostBackend {
        root: PathBuf,
    }

    pub struct HostFile {
        file: fs::File,
    }

    impl HostFile {
        pub(crate) fn open(root: &PathBuf, path: &str) -> Result<Self, Error> {
            let full = root.join(path.trim_start_matches('/'));
            Ok(HostFile {
                file: fs::File::open(&full).map_err(|_| Error::NotFound)?,
            })
        }

        pub(crate) fn create(root: &PathBuf, path: &str) -> Result<Self, Error> {
            let full = root.join(path.trim_start_matches('/'));
            if let Some(parent) = full.parent() {
                let _ = fs::create_dir_all(parent);
            }
            Ok(HostFile {
                file: fs::File::create(&full).map_err(|_| Error::WriteError)?,
            })
        }
    }

    impl File for HostFile {
        fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
            use std::io::Read;
            self.file.read(buf).map_err(|_| Error::ReadError)
        }

        fn write(&mut self, data: &[u8]) -> Result<usize, Error> {
            use std::io::Write;
            self.file.write(data).map_err(|_| Error::WriteError)
        }

        fn seek(&mut self, pos: SeekFrom) -> Result<usize, Error> {
            use std::io::Seek;
            let pos = match pos {
                SeekFrom::Start(o) => std::io::SeekFrom::Start(o as u64),
                SeekFrom::Current(o) => std::io::SeekFrom::Current(o as i64),
                SeekFrom::End(o) => std::io::SeekFrom::End(o as i64),
            };
            self.file.seek(pos).map(|o| o as usize).map_err(|_| Error::ReadError)
        }

        fn stream_position(&mut self) -> Result<usize, Error> {
            self.seek(SeekFrom::Current(0))
        }

        fn length(&self) -> Result<usize, Error> {
            let meta = self.file.metadata().map_err(|_| Error::ReadError)?;
            Ok(meta.len() as usize)
        }
    }

    impl HostBackend {
        pub fn new() -> Self {
            let root = PathBuf::from(ROOT);
            let _ = fs::create_dir_all(&root);
            Self { root }
        }

        pub fn list_dir(
            &self,
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

        pub fn open_file(&self, path: &str) -> Result<HostFile, Error> {
            HostFile::open(&self.root, path)
        }

        pub fn create_file(&self, path: &str) -> Result<HostFile, Error> {
            HostFile::create(&self.root, path)
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

    pub fn list_dir(
        &self,
        path: &str,
        f: &mut dyn FnMut(&str) -> bool,
    ) -> Result<(), Error> {
        self.inner.list_dir(path, f)
    }

    pub fn exists(&self, path: &str) -> bool {
        self.inner.exists(path)
    }

    /// Open a file for reading.
    pub fn open(&self, path: &str) -> Result<impl File, Error> {
        #[cfg(target_arch = "riscv32")]
        {
            self.inner.open_file(path, embedded_sdmmc::Mode::ReadOnly)
        }
        #[cfg(target_arch = "x86_64")]
        {
            self.inner.open_file(path)
        }
    }

    /// Create a file for writing (truncates if exists).
    pub fn create(&self, path: &str) -> Result<impl File, Error> {
        #[cfg(target_arch = "riscv32")]
        {
            self.inner.open_file(path, embedded_sdmmc::Mode::ReadWriteCreateOrTruncate)
        }
        #[cfg(target_arch = "x86_64")]
        {
            self.inner.create_file(path)
        }
    }
}

// ── Tests (desktop backend only) ───────────────────────────────────────

#[cfg(all(test, target_arch = "x86_64"))]
mod tests {
    use super::*;
    use std::fs;

    fn cleanup(path: &str) {
        let _ = fs::remove_file(format!("./sd_root/{}", path));
    }

    #[test]
    fn write_and_read() {
        let s = Storage::new();
        let path = "__test_write_read.bin";
        let data = b"P4\n4 4\n\x00\x00";

        let mut f = s.create(path).unwrap();
        f.write(data).unwrap();
        drop(f);

        assert!(s.exists(path));

        let mut f = s.open(path).unwrap();
        let mut buf = [0u8; 128];
        let n = f.read(&mut buf).unwrap();
        assert_eq!(&buf[..n], data);

        cleanup(path);
    }

    #[test]
    fn list_and_exists() {
        let s = Storage::new();
        s.create("__test_list_a.txt").unwrap().write(b"hello").unwrap();
        s.create("__test_list_b.txt").unwrap().write(b"world").unwrap();

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

        cleanup("__test_list_a.txt");
        cleanup("__test_list_b.txt");
    }

    #[test]
    fn pbm_roundtrip() {
        let s = Storage::new();
        let pbm = b"P4\n8 8\n\xAA\x55\xAA\x55\xAA\x55\xAA\x55";
        s.create("__test_roundtrip.pbm").unwrap().write(pbm).unwrap();

        let mut f = s.open("__test_roundtrip.pbm").unwrap();
        let mut buf = [0u8; 128];
        let n = f.read(&mut buf).unwrap();
        assert_eq!(&buf[..n], pbm);

        cleanup("__test_roundtrip.pbm");
    }

    #[test]
    fn file_open_read() {
        let s = Storage::new();
        let data = b"Hello, world!";
        s.create("__test_file.bin").unwrap().write(data).unwrap();

        let mut f = s.open("__test_file.bin").unwrap();
        let mut buf = [0u8; 32];
        let n = f.read(&mut buf).unwrap();
        assert_eq!(&buf[..n], data);
        assert_eq!(n, 13);

        cleanup("__test_file.bin");
    }

    #[test]
    fn file_seek_tell() {
        let s = Storage::new();
        let data = b"ABCDEFGHIJ";
        s.create("__test_seek.bin").unwrap().write(data).unwrap();

        let mut f = s.open("__test_seek.bin").unwrap();
        assert_eq!(f.stream_position().unwrap(), 0);

        let mut buf = [0u8; 4];
        f.read(&mut buf).unwrap();
        assert_eq!(&buf, b"ABCD");
        assert_eq!(f.stream_position().unwrap(), 4);

        f.seek(SeekFrom::Start(2)).unwrap();
        assert_eq!(f.stream_position().unwrap(), 2);
        f.read(&mut buf).unwrap();
        assert_eq!(&buf, b"CDEF");

        f.seek(SeekFrom::End(0)).unwrap();
        assert_eq!(f.stream_position().unwrap(), 10);

        cleanup("__test_seek.bin");
    }

    #[test]
    fn file_length() {
        let s = Storage::new();
        let data = vec![0xAAu8; 256];
        s.create("__test_len.bin").unwrap().write(&data).unwrap();

        let f = s.open("__test_len.bin").unwrap();
        assert_eq!(f.length().unwrap(), 256);

        cleanup("__test_len.bin");
    }
}
