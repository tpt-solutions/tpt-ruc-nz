// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Device entry point. Reads the GNSS UART, records distance on the device,
//! and prints each finished segment on the USB-Serial-JTAG console.
//!
//! The recording logic lives in `tpt-ruc-firmware`, which is tested on the
//! host. This file only wires it to the chip's UART.

#![no_std]
#![no_main]

use esp_backtrace as _;
use esp_hal::main;
use esp_hal::uart::{Config as UartConfig, Uart};
use esp_println::println;
use tpt_ruc_firmware::Recorder;

esp_bootloader_esp_idf::esp_app_desc!();

/// Segments the recorder can hold before the oldest are dropped.
const QUEUE_CAPACITY: usize = 32;

#[main]
fn main() -> ! {
    // The device code allocates nothing. The heap exists only because a dependency links the allocator.
    esp_alloc::heap_allocator!(size: 8 * 1024);

    let peripherals = esp_hal::init(esp_hal::Config::default());

    // GNSS receiver: 9600 baud, 8N1 (the NEO-M8N default). Pins follow hardware/README.md.
    #[cfg(feature = "esp32s3")]
    let (rx_pin, tx_pin) = (peripherals.GPIO16, peripherals.GPIO17);
    #[cfg(feature = "esp32c6")]
    let (rx_pin, tx_pin) = (peripherals.GPIO4, peripherals.GPIO5);
    // ESP32-P4 pins are not yet in hardware/README.md. These are placeholders to confirm before use.
    #[cfg(feature = "esp32p4")]
    let (rx_pin, tx_pin) = (peripherals.GPIO16, peripherals.GPIO17);

    let mut gnss = Uart::new(peripherals.UART1, UartConfig::default().with_baudrate(9600))
        .expect("UART1 accepts the default configuration")
        .with_rx(rx_pin)
        .with_tx(tx_pin);

    let mut recorder: Recorder<QUEUE_CAPACITY> = Recorder::new();
    let mut buf = [0u8; 64];
    println!("tpt-ruc-nz recorder started on {}", tpt_ruc_firmware::CHIP);

    loop {
        if let Ok(n) = gnss.read(&mut buf) {
            for &byte in &buf[..n] {
                recorder.push_byte(byte);
            }
        }
        while let Some(segment) = recorder.next_segment() {
            println!(
                "segment start_ms={} end_ms={} distance_m={}",
                segment.start_ms, segment.end_ms, segment.distance_m
            );
        }
    }
}
