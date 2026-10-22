// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! The BLE GATT server: live fix, segment transfer, control and device info.
//!
//! The layout is in `docs/ble-design.md`, and the rules (what a write does, how a
//! transfer is acknowledged) are in `tpt-ruc-firmware`. This module only does the
//! radio and ATT work. It holds the device lock for as short a time as it can, and
//! never across a notification.
//!
//! Nothing here has run on a board. The host tests cover the logic, not the radio.

use embassy_futures::select::{select, Either};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_time::{Duration, Instant, Timer};
use esp_println::println;
use esp_radio::ble::controller::BleConnector;
use tpt_ruc_firmware::control::RESP_INVALID;
use tpt_ruc_firmware::gatt::Action;
use tpt_ruc_firmware::payload::encode_fix_v1;
use trouble_host::prelude::*;

use crate::device::INFO_LEN;
use crate::DEVICE;

/// Connections the server accepts at once. The design allows one phone.
const CONNECTIONS_MAX: usize = 1;
/// L2CAP channels for the host. Three is what the ATT and signalling channels need.
const L2CAP_CHANNELS_MAX: usize = 3;
/// Attribute table slots. Enough for the tracker service and the standard services.
const ATT_MAX: usize = 64;
/// Largest chunk. The design asks for an MTU of 247, which leaves 244 bytes for a notification.
const MAX_CHUNK: usize = 244;
/// Largest live fix payload. A version 1 payload is at most 31 bytes.
const LIVE_LEN: usize = 32;
/// Largest control write. The longest command is five bytes.
const CONTROL_LEN: usize = 8;
/// The advertised name is `tpt-ruc-` followed by four hex digits.
pub const NAME_LEN: usize = 12;
/// Smallest gap between live fix notifications.
const LIVE_INTERVAL: Duration = Duration::from_secs(1);

/// Controller type: esp-radio's BLE connector, through bt-hci.
type Controller = ExternalController<BleConnector<'static>, 20>;

#[gatt_server(connections_max = CONNECTIONS_MAX, mutex_type = NoopRawMutex, attribute_table_size = ATT_MAX)]
struct Server {
    tracker: TrackerService,
}

#[gatt_service(uuid = "7b0e0001-5f4c-4a51-9c3d-7a6b5c4d3e2f")]
struct TrackerService {
    /// Live fix: one version 1 payload per accepted fix, at most once a second.
    #[characteristic(uuid = "7b0e0002-5f4c-4a51-9c3d-7a6b5c4d3e2f", read, notify)]
    live: heapless::Vec<u8, LIVE_LEN>,
    /// Segment transfer: record chunks, oldest first.
    #[characteristic(uuid = "7b0e0003-5f4c-4a51-9c3d-7a6b5c4d3e2f", notify)]
    segments: heapless::Vec<u8, MAX_CHUNK>,
    /// Control: commands from the phone. Status bytes come back as notifications.
    #[characteristic(uuid = "7b0e0004-5f4c-4a51-9c3d-7a6b5c4d3e2f", write, notify)]
    control: heapless::Vec<u8, CONTROL_LEN>,
    /// Device info: firmware, payload version, used and total log bytes.
    #[characteristic(uuid = "7b0e0005-5f4c-4a51-9c3d-7a6b5c4d3e2f", read)]
    info: heapless::Vec<u8, INFO_LEN>,
}

/// The advertised name and the random address, both from the device's public key.
///
/// The name is `tpt-ruc-` and the last two key bytes in hex. The address is the
/// first six key bytes, with the two top bits set, which makes it a static random address.
pub fn name_and_address(public_key: &[u8; 32]) -> ([u8; NAME_LEN], [u8; 6]) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut name = [0u8; NAME_LEN];
    name[..8].copy_from_slice(b"tpt-ruc-");
    for (i, byte) in public_key[30..].iter().enumerate() {
        name[8 + 2 * i] = HEX[usize::from(byte >> 4)];
        name[9 + 2 * i] = HEX[usize::from(byte & 0x0f)];
    }
    let mut address = [0u8; 6];
    address.copy_from_slice(&public_key[..6]);
    address[5] |= 0xc0;
    (name, address)
}

/// Runs the BLE stack and the GATT server until the program stops. Spawned once at boot.
#[embassy_executor::task]
pub async fn run(connector: BleConnector<'static>, name: [u8; NAME_LEN], address: [u8; 6]) {
    let controller: Controller = ExternalController::new(connector);
    let mut resources: HostResources<
        Controller,
        DefaultPacketPool,
        CONNECTIONS_MAX,
        L2CAP_CHANNELS_MAX,
    > = HostResources::new();
    let stack = trouble_host::new(controller, &mut resources)
        .set_random_address(Address::random(address))
        .build();
    let mut runner = stack.runner();
    let mut peripheral = stack.peripheral();

    let name = core::str::from_utf8(&name).unwrap_or("tpt-ruc");
    let config = GapConfig::Peripheral(PeripheralConfig {
        name,
        appearance: &appearance::power_device::GENERIC_POWER_DEVICE,
    });
    let server = match Server::new_with_config(config) {
        Ok(server) => server,
        Err(_) => {
            println!("ble: the GATT server could not be built");
            return;
        }
    };

    select(
        runner.run(),
        advertise_forever(&mut peripheral, &server, name.as_bytes()),
    )
    .await;
}

/// Advertises, accepts one phone, and serves it. Starts again after each connection ends.
async fn advertise_forever(
    peripheral: &mut Peripheral<'_, Controller, DefaultPacketPool>,
    server: &Server<'_>,
    name: &[u8],
) {
    let mut adv_data = [0u8; 31];
    let adv_len = AdStructure::encode_slice(
        &[AdStructure::Flags(
            LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED,
        )],
        &mut adv_data,
    )
    .unwrap_or(0);
    let mut scan_data = [0u8; 31];
    let scan_len =
        AdStructure::encode_slice(&[AdStructure::CompleteLocalName(name)], &mut scan_data)
            .unwrap_or(0);

    loop {
        if advertise_once(
            peripheral,
            server,
            &adv_data[..adv_len],
            &scan_data[..scan_len],
        )
        .await
        .is_err()
        {
            // Give the stack a moment before advertising again.
            Timer::after(Duration::from_secs(1)).await;
        }
    }
}

async fn advertise_once(
    peripheral: &mut Peripheral<'_, Controller, DefaultPacketPool>,
    server: &Server<'_>,
    adv_data: &[u8],
    scan_data: &[u8],
) -> Result<(), ()> {
    let advertiser = peripheral
        .advertise(
            &Default::default(),
            Advertisement::ConnectableScannableUndirected {
                adv_data,
                scan_data,
            },
        )
        .await
        .map_err(|_| ())?;
    let conn = advertiser
        .accept()
        .await
        .map_err(|_| ())?
        .with_attribute_server(server)
        .map_err(|_| ())?;
    println!("ble: connected");
    let result = serve(&conn, server).await;
    println!("ble: disconnected");
    result
}

/// Serves one connection until the phone disconnects, or the link fails.
async fn serve(
    conn: &GattConnection<'_, '_, DefaultPacketPool>,
    server: &Server<'_>,
) -> Result<(), ()> {
    let mut sent_fix: Option<u64> = None;
    let mut last_live: Option<Instant> = None;
    let mut chunk = [0u8; MAX_CHUNK];
    loop {
        publish_live(conn, server, &mut sent_fix, &mut last_live).await?;
        let busy = send_chunk(conn, server, &mut chunk).await?;
        // While a transfer has chunks left, look for events without waiting. Otherwise wait for one,
        // and wake in time to publish the next live fix.
        let event = if busy {
            match select(conn.next(), embassy_futures::yield_now()).await {
                Either::First(event) => Some(event),
                Either::Second(()) => None,
            }
        } else {
            match select(conn.next(), Timer::after(LIVE_INTERVAL)).await {
                Either::First(event) => Some(event),
                Either::Second(()) => None,
            }
        };
        if let Some(event) = event {
            match event {
                GattConnectionEvent::Disconnected { .. } => return Ok(()),
                GattConnectionEvent::Gatt { event } => handle_gatt(conn, server, event).await?,
                _ => {}
            }
        }
    }
}

/// Answers one GATT request. Writes to the control characteristic are applied here.
async fn handle_gatt(
    conn: &GattConnection<'_, '_, DefaultPacketPool>,
    server: &Server<'_>,
    event: GattEvent<'_, '_, DefaultPacketPool>,
) -> Result<(), ()> {
    match event {
        GattEvent::Write(write) if write.handle() == server.tracker.control.handle => {
            let mut bytes: heapless::Vec<u8, CONTROL_LEN> = heapless::Vec::new();
            let fits = write.with_data(|_, data| bytes.extend_from_slice(data).is_ok());
            let (status, refused) = if fits {
                control_write(conn, server, &bytes).await
            } else {
                (RESP_INVALID, None)
            };
            write.accept().map_err(|_| ())?.send().await;
            server
                .tracker
                .control
                .notify_raw(conn, &[status], false)
                .await
                .map_err(|_| ())?;
            if let Some(status) = refused {
                server
                    .tracker
                    .control
                    .notify_raw(conn, &[status], false)
                    .await
                    .map_err(|_| ())?;
            }
            Ok(())
        }
        GattEvent::Write(write) => {
            write.accept().map_err(|_| ())?.send().await;
            Ok(())
        }
        GattEvent::Read(read) => {
            read.accept().map_err(|_| ())?.send().await;
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Applies one control write under the device lock. Returns the status to notify, and a second
/// status when the device refused a transfer step after accepting the command.
async fn control_write(
    conn: &GattConnection<'_, '_, DefaultPacketPool>,
    server: &Server<'_>,
    bytes: &[u8],
) -> (u8, Option<u8>) {
    let mtu = conn.raw().att_mtu();
    let mut guard = DEVICE.lock().await;
    let Some(device) = guard.as_mut() else {
        return (RESP_INVALID, None);
    };
    let reply = device.control(bytes);
    let refused = match reply.action {
        Action::RequestSegments => device.start_transfer(mtu).is_err(),
        Action::AcknowledgeSegments(count) => device.acknowledge(count).is_err(),
        Action::None | Action::SettingsChanged(_) => false,
    };
    let info = device.info();
    drop(guard);

    if let Ok(info) = heapless::Vec::<u8, INFO_LEN>::from_slice(&info) {
        server.set(&server.tracker.info, &info).ok();
    }
    (reply.status, refused.then_some(RESP_INVALID))
}

/// Sends the next chunk of an open transfer. Returns whether the transfer has more to send.
async fn send_chunk(
    conn: &GattConnection<'_, '_, DefaultPacketPool>,
    server: &Server<'_>,
    chunk: &mut [u8; MAX_CHUNK],
) -> Result<bool, ()> {
    let next = {
        let mut guard = DEVICE.lock().await;
        let Some(device) = guard.as_mut() else {
            return Ok(false);
        };
        // A chunk that fails closes the transfer inside the device, so the phone asks again.
        device.next_chunk(chunk).unwrap_or(None)
    };
    match next {
        Some(len) => {
            server
                .tracker
                .segments
                .notify_raw(conn, &chunk[..len], false)
                .await
                .map_err(|_| ())?;
            Ok(true)
        }
        None => Ok(false),
    }
}

/// Sends the newest live fix, if it is new and the last one was at least [`LIVE_INTERVAL`] ago.
async fn publish_live(
    conn: &GattConnection<'_, '_, DefaultPacketPool>,
    server: &Server<'_>,
    sent_fix: &mut Option<u64>,
    last_live: &mut Option<Instant>,
) -> Result<(), ()> {
    if last_live.is_some_and(|at| at.elapsed() < LIVE_INTERVAL) {
        return Ok(());
    }
    let fix = {
        let guard = DEVICE.lock().await;
        guard.as_ref().and_then(|device| device.live_fix())
    };
    let Some(fix) = fix else {
        return Ok(());
    };
    if *sent_fix == Some(fix.time_ms) {
        return Ok(());
    }
    let mut payload = [0u8; LIVE_LEN];
    let len = encode_fix_v1(&fix, &mut payload).map_err(|_| ())?;
    let value = heapless::Vec::<u8, LIVE_LEN>::from_slice(&payload[..len]).map_err(|_| ())?;
    server
        .tracker
        .live
        .notify(conn, &value, true)
        .await
        .map_err(|_| ())?;
    *sent_fix = Some(fix.time_ms);
    *last_live = Some(Instant::now());
    Ok(())
}
