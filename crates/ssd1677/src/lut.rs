// Waveform lookup table types for SSD1677.
// See datasheet Section 6.6-6.7 for the full waveform format.
// 10 groups × 4 phases × 5 voltage selections + 10 repeat counts + 5 frame rate bytes = 105 bytes total.

use core::cell::Cell;

/// 105-byte waveform LUT buffer for Register 0x32.
pub type LutBuffer = Cell<[u8; 105]>;

/// Source and VCOM voltage selection (Table 6-6).
#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub enum Voltage {
    /// VSS (0V), DCVCOM — balanced, no migration
    Vss = 0b00,
    /// VSH1, VSH1+DCVCOM — positive drive
    Vsh1 = 0b01,
    /// VSL, VSL+DCVCOM — negative drive
    Vsl = 0b10,
    /// VSH2 — strong positive (no VCOM)
    Vsh2 = 0b11,
}

/// Voltage selections for all 5 LUTs in a single phase.
#[derive(Clone, Copy, Debug)]
pub struct PhaseVoltages {
    /// LUT0: drives black pixels (B&W mode)
    pub lut0: Voltage,
    /// LUT1: drives white pixels (B&W mode)
    pub lut1: Voltage,
    /// LUT2: redundant in B&W (=LUT0), red pixels in 3-color
    pub lut2: Voltage,
    /// LUT3: redundant in B&W (=LUT1), red in 3-color
    pub lut3: Voltage,
    /// LUT4: VCOM reference waveform
    pub lut4: Voltage,
}

/// One phase within a waveform group.
#[derive(Clone, Copy, Debug)]
pub struct Phase {
    /// Voltage selections for this phase.
    pub voltages: PhaseVoltages,
    /// Frame count (TP). 0 = skip phase, 1-255 = number of frames.
    pub frames: u8,
}

/// One waveform group: 4 phases with a repeat count.
#[derive(Clone, Copy, Debug)]
pub struct Group {
    /// Phases A, B, C, D.
    pub phases: [Phase; 4],
    /// Repeat count (RP). 0 = run once, 255 = run 256 times.
    pub repeat: u8,
}

/// Complete waveform lookup table for SSD1677.
///
/// Serialized as 105 bytes via Register 0x32.
/// Unused groups (set to `None`) are serialized with zero frames and repeats.
#[derive(Clone, Debug)]
pub struct WaveformLut {
    /// Active groups. `None` groups are skipped (TP=0, RP=0 in output).
    pub groups: [Option<Group>; 10],
    /// Frame rate timing values. Panel-specific, copy from OTP.
    pub frame_rate: [u8; 5],
}

impl WaveformLut {
    /// Serialize into a 105-byte buffer for Register 0x32.
    pub fn to_buffer(&self) -> LutBuffer {
        let buf = Cell::new([0u8; 105]);
        let bytes = buf.as_array_of_cells();
        let mut idx = 0;

        // Bytes 0-49: VS[nX-LUTm]
        // Each byte encodes 4 phases for one LUT, from MSB to LSB: A B C D.
        // LUT0: bytes 0-9, LUT1: 10-19, LUT2: 20-29, LUT3: 30-39, LUT4: 40-49.
        for lut_idx in 0..5u8 {
            for group in &self.groups {
                let phases = group.as_ref().map(|g| g.phases).unwrap_or([Phase {
                    voltages: PhaseVoltages { lut0: Voltage::Vss, lut1: Voltage::Vss, lut2: Voltage::Vss, lut3: Voltage::Vss, lut4: Voltage::Vss },
                    frames: 0,
                }; 4]);
                let byte: u8 = (phase_voltage(lut_idx, phases[0].voltages) << 6)
                    | (phase_voltage(lut_idx, phases[1].voltages) << 4)
                    | (phase_voltage(lut_idx, phases[2].voltages) << 2)
                    | phase_voltage(lut_idx, phases[3].voltages);
                bytes[idx].set(byte);
                idx += 1;
            }
        }

        // Bytes 50-89: TP[nX] — 40 phase lengths (A,B,C,D for 10 groups)
        for group in &self.groups {
            let phases = group.as_ref().map(|g| g.phases).unwrap_or([Phase {
                voltages: PhaseVoltages { lut0: Voltage::Vss, lut1: Voltage::Vss, lut2: Voltage::Vss, lut3: Voltage::Vss, lut4: Voltage::Vss },
                frames: 0,
            }; 4]);
            for phase in &phases {
                bytes[idx].set(phase.frames);
                idx += 1;
            }
        }

        // Bytes 90-99: RP[n] — 10 repeat counts
        for group in &self.groups {
            bytes[idx].set(group.map(|g| g.repeat).unwrap_or(0));
            idx += 1;
        }

        // Bytes 100-104: frame rate
        for (i, &b) in self.frame_rate.iter().enumerate() {
            bytes[100 + i].set(b);
        }

        buf
    }
}

/// Extract the 2-bit voltage for a specific LUT index from PhaseVoltages.
fn phase_voltage(lut_idx: u8, pv: PhaseVoltages) -> u8 {
    (match lut_idx {
        0 => pv.lut0,
        1 => pv.lut1,
        2 => pv.lut2,
        3 => pv.lut3,
        4 => pv.lut4,
        _ => Voltage::Vss,
    }) as u8
}
