// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Device entry point. Reads the GNSS UART, records distance on the device, and
//! keeps the segments in flash until the phone takes them over BLE.
//!
//! The recording logic lives in `tpt-ruc-firmware`, which is tested on the host.
//! This file and its modules only wire it to the chip. The `ble` feature is on for
//! ESP32-C6 and ESP32-S3. The ESP32-P4 has no radio and builds without it.

#![no_std]
#![no_main]

mod device;

#[cfg(feature = "ble")]
mod ble;

use esp_backtrace as _;
use esp_hal::main;
#[cfg(feature = "esp32p4")]
use esp_hal::rng::Rng;
#[cfg(not(feature = "esp32p4"))]
use esp_hal::rng::{Trng, TrngSource};
use esp_hal::time::Duration;
#[cfg(feature = "ble")]
use esp_hal::timer::timg::TimerGroup;
use esp_hal::timer::timg::{MwdtStage, Wdt};
use esp_hal::uart::{Config as UartConfig, Uart};
use esp_println::println;

use device::{Device, EspRandom, GnssTx};

esp_bootloader_esp_idf::esp_app_desc!();

/// Seconds without GNSS bytes before the watchdog resets the device.
const WATCHDOG_SECONDS: u64 = 10;

#[cfg(feature = "ble")]
static DEVICE: embassy_sync::mutex::Mutex<
    embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
    Option<Device>,
> = embassy_sync::mutex::Mutex::new(None);

#[main]
fn main() -> ! {
    // The BLE stack allocates, so it gets a larger heap. Without BLE nothing allocates.
    #[cfg(feature = "ble")]
    esp_alloc::heap_allocator!(size: 72 * 1024);
    #[cfg(not(feature = "ble"))]
    esp_alloc::heap_allocator!(size: 8 * 1024);

    let peripherals = esp_hal::init(esp_hal::Config::default());

    // The TRNG draws on the ADC's noise, so its source stays alive while the seed is read.
    #[cfg(not(feature = "esp32p4"))]
    let _trng_source = TrngSource::new(peripherals.RNG, peripherals.ADC1);
    #[cfg(not(feature = "esp32p4"))]
    let rng = EspRandom(Trng::try_new().expect("the hardware RNG is enabled"));
    #[cfg(feature = "esp32p4")]
    let rng = EspRandom(Rng::new());

    let flash = esp_storage::FlashStorage::new(peripherals.FLASH);
    let (log, identity) = device::open(flash, rng).expect("the device storage opens");
    println!("device key {:02x?}", identity.public_key());

    // Watchdog: fed each time GNSS bytes arrive, so a silent receiver resets the device.
    let mut wdt = Wdt::<esp_hal::peripherals::TIMG1<'static>>::new();
    wdt.set_timeout(MwdtStage::Stage0, Duration::from_secs(WATCHDOG_SECONDS));
    wdt.enable();

    // GNSS receiver: 9600 baud, 8N1 (the NEO-M8N default). Pins follow hardware/README.md.
    #[cfg(feature = "esp32s3")]
    let (rx_pin, tx_pin) = (peripherals.GPIO16, peripherals.GPIO17);
    #[cfg(feature = "esp32c6")]
    let (rx_pin, tx_pin) = (peripherals.GPIO4, peripherals.GPIO5);
    // ESP32-P4 pins are not yet in hardware/README.md. These are placeholders to confirm before use.
    #[cfg(feature = "esp32p4")]
    let (rx_pin, tx_pin) = (peripherals.GPIO16, peripherals.GPIO17);

    // The transmit half stays blocking, because UBX frames are written from synchronous code.
    // The receive half is split off and made async for the BLE build.
    let (rx, tx) = Uart::new(peripherals.UART1, UartConfig::default().with_baudrate(9600))
        .expect("UART1 accepts the default configuration")
        .with_rx(rx_pin)
        .with_tx(tx_pin)
        .split();

    println!("tpt-ruc-nz recorder started on {}", tpt_ruc_firmware::CHIP);

    #[cfg(feature = "ble")]
    {
        // The device is ready before the first GNSS byte, so no segment is lost to a race.
        DEVICE
            .try_lock()
            .expect("the device is not yet shared")
            .replace(Device::new(log, GnssTx(tx)));

        let timg0 = TimerGroup::new(peripherals.TIMG0);
        esp_rtos::start(timg0.timer0, peripherals.FROM_CPU_INTR0);

        let connector =
            esp_radio::ble::controller::BleConnector::new(peripherals.BT, Default::default())
                .expect("the BLE controller starts");
        let (name, address) = ble::name_and_address(&identity.public_key());

        static mut EXECUTOR: esp_rtos::embassy::Executor = esp_rtos::embassy::Executor::new();
        // SAFETY: `main` runs once, so this is the only place that takes the executor.
        let executor = unsafe { &mut *core::ptr::addr_of_mut!(EXECUTOR) };
        executor.run(|spawner| {
            spawner.spawn(gnss_task(rx.into_async(), wdt).expect("the GNSS task is free"));
            spawner.spawn(ble::run(connector, name, address).expect("the BLE task is free"));
        });
    }

    #[cfg(not(feature = "ble"))]
    {
        let _ = identity;
        let mut device = Device::new(log, GnssTx(tx));
        let mut rx = rx;
        let mut buf = [0u8; 64];
        loop {
            if let Ok(n) = rx.read(&mut buf) {
                wdt.feed();
                device.push_gnss(&buf[..n]);
            }
        }
    }
}

/// Reads GNSS bytes and feeds them to the recorder. Runs for the life of the device.
#[cfg(feature = "ble")]
#[embassy_executor::task]
async fn gnss_task(
    mut rx: esp_hal::uart::UartRx<'static, esp_hal::Async>,
    mut wdt: Wdt<esp_hal::peripherals::TIMG1<'static>>,
) {
    let mut buf = [0u8; 64];
    loop {
        // A read error loses only these bytes. The stream picks up at the next read.
        let Ok(n) = rx.read_async(&mut buf).await else {
            continue;
        };
        wdt.feed();
        if let Some(device) = DEVICE.lock().await.as_mut() {
            device.push_gnss(&buf[..n]);
        }
    }
}
