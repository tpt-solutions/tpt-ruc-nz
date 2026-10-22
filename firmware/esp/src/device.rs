// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Chip glue shared by every build: flash, the device seed, and the device state.
//!
//! The logic is in `tpt-ruc-firmware`, which is tested on the host. This module
//! only connects it to the chip's flash, hardware RNG and GNSS UART.

#![cfg_attr(not(feature = "ble"), allow(dead_code))]

use esp_bootloader_esp_idf::partitions::read_partition_table;
use esp_hal::uart::UartTx;
use esp_hal::Blocking;
use esp_storage::FlashStorage;
use tpt_ruc_core::Fix;
use tpt_ruc_firmware::control::Settings;
use tpt_ruc_firmware::flash::SectorLog;
use tpt_ruc_firmware::gatt::{self, GnssSink, Reply};
use tpt_ruc_firmware::identity::{load_identity, DeviceIdentity};
use tpt_ruc_firmware::nor::NorSectors;
use tpt_ruc_firmware::queue::drain_into;
use tpt_ruc_firmware::recorder::Recorder;
use tpt_ruc_firmware::seed::{FlashSeed, RandomSource, SeedError};
use tpt_ruc_firmware::session::{SessionError, TransferSession};

/// Label of the flash region for the seed and the trip log. Must match `partitions.csv`.
pub const PARTITION: &str = "tpt-ruc";

/// Erase sector of the chip's flash.
const SECTOR: u32 = 4096;

/// Size of the partition table in flash. The bootloader's table is one 3 KiB block.
const PARTITION_TABLE_LEN: usize = 0xc00;

/// Segments the recorder holds before the oldest are dropped.
pub const QUEUE_CAPACITY: usize = 32;

/// Segments read from flash for each batch of a transfer.
pub const TRANSFER_BATCH: usize = 4;

/// Firmware version reported on the device info characteristic.
pub const FW_VERSION: u8 = 1;

/// Length of the device info value: firmware, payload, used bytes and total bytes.
pub const INFO_LEN: usize = 10;

/// The flash the log and seed live in.
pub type Sectors = NorSectors<FlashStorage<'static>>;

/// Why the device could not start its storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootError {
    /// The partition table could not be read.
    PartitionTable,
    /// No partition labelled [`PARTITION`] exists in the table.
    NoPartition,
    /// The partition does not fit the storage layout (not on a sector boundary, or too small).
    Layout,
    /// The seed could not be loaded or made.
    Seed(SeedError),
}

/// The chip's random source for the seed. The ESP32-P4 has no TRNG in esp-hal, so it uses
/// the plain `Rng` there. That source is weaker, and the P4 build is not a release target.
#[cfg(not(feature = "esp32p4"))]
pub type Random = esp_hal::rng::Trng;
#[cfg(feature = "esp32p4")]
pub type Random = esp_hal::rng::Rng;

/// The seed's random source, backed by [`Random`].
pub struct EspRandom(pub Random);

impl RandomSource for EspRandom {
    fn fill(&mut self, buf: &mut [u8]) {
        self.0.read(buf);
    }
}

/// The GNSS UART's transmit half, for the UBX frames that change the receiver's rate.
pub struct GnssTx(pub UartTx<'static, Blocking>);

impl GnssSink for GnssTx {
    fn write_all(&mut self, mut bytes: &[u8]) -> bool {
        while !bytes.is_empty() {
            match self.0.write(bytes) {
                Ok(0) | Err(_) => return false,
                Ok(written) => bytes = &bytes[written..],
            }
        }
        self.0.flush().is_ok()
    }
}

/// Opens the trip log and loads the device identity from the seed in the same partition.
///
/// The seed is loaded first, so the identity exists before any segment is written.
pub fn open(
    mut flash: FlashStorage<'static>,
    rng: EspRandom,
) -> Result<(SectorLog<Sectors>, DeviceIdentity), BootError> {
    let mut table_buf = [0u8; PARTITION_TABLE_LEN];
    let table =
        read_partition_table(&mut flash, &mut table_buf).map_err(|_| BootError::PartitionTable)?;
    let entry = table
        .iter()
        .find(|entry| entry.label_as_str() == PARTITION)
        .ok_or(BootError::NoPartition)?;

    let sectors = (entry.len() / SECTOR) as usize;
    let mut sectors = NorSectors::new(flash, entry.offset(), sectors).ok_or(BootError::Layout)?;
    let identity =
        load_identity(&mut FlashSeed::new(&mut sectors, rng)).map_err(BootError::Seed)?;
    let log = SectorLog::open(sectors.after(1).ok_or(BootError::Layout)?);
    Ok((log, identity))
}

/// Everything the device keeps between events: the recorder, the flash log, the
/// settings, the open transfer, and the GNSS transmitter.
pub struct Device {
    pub recorder: Recorder<QUEUE_CAPACITY>,
    pub log: SectorLog<Sectors>,
    pub settings: Settings,
    pub session: Option<TransferSession<TRANSFER_BATCH>>,
    pub gnss: GnssTx,
}

impl Device {
    /// Starts with default settings, no transfer, and an empty recorder.
    pub fn new(log: SectorLog<Sectors>, gnss: GnssTx) -> Self {
        Self {
            recorder: Recorder::new(),
            log,
            settings: Settings::default(),
            session: None,
            gnss,
        }
    }

    /// Feeds GNSS bytes to the recorder, then moves finished segments into flash.
    pub fn push_gnss(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.recorder.push_byte(byte);
        }
        drain_into(self.recorder.queue_mut(), &mut self.log);
    }

    /// The newest fix from the receiver, for the live view.
    pub fn live_fix(&self) -> Option<Fix> {
        self.recorder.latest_fix()
    }

    /// Applies one control write. An accepted rate reaches the receiver before the reply is sent.
    pub fn control(&mut self, bytes: &[u8]) -> Reply {
        gatt::apply_control_write(
            &mut self.settings,
            &mut self.recorder,
            &mut self.gnss,
            bytes,
        )
    }

    /// Starts a transfer of the log, oldest segment first, at the given ATT MTU.
    pub fn start_transfer(&mut self, mtu: u16) -> Result<(), SessionError> {
        self.session = Some(TransferSession::start(&self.log, mtu)?);
        Ok(())
    }

    /// Writes the next chunk of the open transfer into `out`. Returns its length, or `None`
    /// when the transfer is finished or none is open. A failed chunk closes the transfer,
    /// so the phone has to ask again.
    pub fn next_chunk(&mut self, out: &mut [u8]) -> Result<Option<usize>, SessionError> {
        let Some(session) = self.session.as_mut() else {
            return Ok(None);
        };
        let result = session.next_chunk(&self.log, out);
        if !matches!(result, Ok(Some(_))) {
            self.session = None;
        }
        result
    }

    /// Records that the phone holds the first `count` segments of the open transfer.
    /// Acknowledging with no transfer open is refused.
    pub fn acknowledge(&mut self, count: u32) -> Result<(), SessionError> {
        let session = self
            .session
            .as_mut()
            .ok_or(SessionError::AckAheadOfSent { sent: 0 })?;
        session.acknowledge(&mut self.log, count)?;
        Ok(())
    }

    /// The device info value: firmware version, payload version, used bytes and total bytes (both `u32` LE).
    pub fn info(&self) -> [u8; INFO_LEN] {
        let sector = self.log.sector_size();
        let used = (self.log.live_sectors() * sector) as u32;
        let total = (self.log.sector_count() * sector) as u32;
        let mut info = [0u8; INFO_LEN];
        info[0] = FW_VERSION;
        info[1] = tpt_ruc_firmware::payload::PAYLOAD_VERSION;
        info[2..6].copy_from_slice(&used.to_le_bytes());
        info[6..10].copy_from_slice(&total.to_le_bytes());
        info
    }
}
