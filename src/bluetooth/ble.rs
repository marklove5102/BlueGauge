use super::{BT_INFO_MAP, info::{BluetoothInfo, BluetoothType}};
use crate::{
    UserEvent,
    notify::NotifyEvent,
};

use std::collections::{
    HashMap, HashSet,
    hash_map::Entry::{Occupied, Vacant},
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use anyhow::{Context, Result, anyhow};
use dashmap::DashMap;
use futures::{StreamExt, future::join_all};
use log::{info, warn};
use tokio::{
    sync::{Mutex, mpsc::Sender},
    time::{Duration, Instant},
};
use windows::{
    Devices::Bluetooth::{
        BluetoothConnectionStatus, BluetoothLEDevice,
        GenericAttributeProfile::{
            GattCharacteristic, GattCharacteristicProperties, GattCharacteristicUuids,
            GattServiceUuids, GattValueChangedEventArgs,
        },
    },
    Devices::Enumeration::DeviceInformation,
    Foundation::TypedEventHandler,
    Storage::Streams::DataReader,
    core::GUID,
};
use winit::event_loop::EventLoopProxy;

pub async fn find_ble_devices() -> Result<Vec<BluetoothLEDevice>> {
    let ble_aqs_filter = BluetoothLEDevice::GetDeviceSelectorFromPairingState(true)?;

    let ble_devices_info = DeviceInformation::FindAllAsyncAqsFilter(&ble_aqs_filter)?
        .await
        .with_context(|| "Failed to find BLE devices info from AqsFilter")?;

    let ble_devices = futures::stream::iter(ble_devices_info)
        .filter_map(|device_info| async move {
            let device_id = device_info.Id().ok()?;
            BluetoothLEDevice::FromIdAsync(&device_id).ok()?.await.ok()
        })
        .collect::<Vec<_>>()
        .await;

    Ok(ble_devices)
}

async fn get_ble_device_from_address(address: u64) -> Result<BluetoothLEDevice> {
    BluetoothLEDevice::FromBluetoothAddressAsync(address)?
        .await
        .with_context(|| format!("Failed to get BLE Device from Address({address})"))
}

pub async fn get_ble_devices_info(
    ble_devices: &[BluetoothLEDevice],
) -> Result<DashMap<u64, BluetoothInfo>> {
    let devices_info: DashMap<u64, BluetoothInfo> = DashMap::new();

    let futures = ble_devices.iter().map(process_ble_device);

    let results = join_all(futures).await;

    results.into_iter().for_each(|result| match result {
        Ok(info) => {
            devices_info.insert(info.address, info);
        }
        Err(e) => warn!("{e}"),
    });

    Ok(devices_info)
}

pub async fn process_ble_device(ble_device: &BluetoothLEDevice) -> Result<BluetoothInfo> {
    let name = ble_device.Name()?.to_string();

    let status = ble_device
        .ConnectionStatus()
        .map(|status| status == BluetoothConnectionStatus::Connected)
        .with_context(|| "Failed to get BLE connected status")?;

    let address = ble_device
        .BluetoothAddress()
        .with_context(|| "Failed to get BLE address")?;

    let battery = get_ble_battery_level(ble_device)
        .await
        .map_err(|e| anyhow!("Failed to get BLE Battery Level: {e}"))?;

    Ok(BluetoothInfo {
        name,
        battery,
        status,
        address,
        r#type: BluetoothType::LowEnergy,
    })
}

async fn get_ble_battery_gatt_char(ble_device: &BluetoothLEDevice) -> Result<GattCharacteristic> {
    // 0000180F-0000-1000-8000-00805F9B34FB
    let battery_services_uuid: GUID = GattServiceUuids::Battery()?;
    // 00002A19-0000-1000-8000-00805F9B34FB
    let battery_level_uuid: GUID = GattCharacteristicUuids::BatteryLevel()?;

    let battery_gatt_services = ble_device
        .GetGattServicesForUuidAsync(battery_services_uuid)?
        .await?
        .Services()
        .map_err(|e| anyhow!("Failed to get BLE Battery Gatt Services: {e}"))?;

    let battery_gatt_service = battery_gatt_services
        .into_iter()
        .next()
        .ok_or(anyhow!("Failed to get BLE Battery Gatt Service"))?; // [*] 手机蓝牙无电量服务;

    let battery_gatt_chars = battery_gatt_service
        .GetCharacteristicsForUuidAsync(battery_level_uuid)?
        .await?
        .Characteristics()
        .map_err(|e| anyhow!("Failed to get BLE Battery Gatt Characteristics: {e}"))?;

    let battery_gatt_char = battery_gatt_chars
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("Failed to get BLE Battery Gatt Characteristic"))?;

    let battery_gatt_char_uuid = battery_gatt_char.Uuid()?;

    if battery_gatt_char_uuid == battery_level_uuid {
        Ok(battery_gatt_char)
    } else {
        Err(anyhow!(
            "Failed to match BLE level UUID:\n{battery_gatt_char_uuid:?}:\n{battery_level_uuid:?}"
        ))
    }
}

async fn get_ble_battery_level(ble_device: &BluetoothLEDevice) -> Result<u8> {
    let battery_gatt_char = get_ble_battery_gatt_char(ble_device).await?;
    let buffer = battery_gatt_char.ReadValueAsync()?.await?.Value()?;
    let reader = DataReader::FromBuffer(&buffer)?;
    reader
        .ReadByte()
        .with_context(|| "Failed to read battery byte")
}

fn get_ble_devices_address<C: FromIterator<u64>>() -> C {
    BT_INFO_MAP
        .iter()
        .filter_map(|entry| entry.is_ble().then_some(*entry.key()))
        .collect()
}

#[derive(Debug)]
enum BluetoothLEUpdate {
    BatteryLevel(/* Address */ u64, u8),
    ConnectionStatus(/* Address */ u64, bool),
}

type WatchBLEGuard = (BluetoothLEDevice, GattCharacteristic, i64, i64);

async fn watch_ble_device(
    ble_address: u64,
    ble_device: BluetoothLEDevice,
    tx: Sender<BluetoothLEUpdate>,
) -> Result<WatchBLEGuard> {
    let battery_gatt_char = get_ble_battery_gatt_char(&ble_device).await?;

    let char_properties = battery_gatt_char.CharacteristicProperties()?;

    if !char_properties.contains(GattCharacteristicProperties::Notify) {
        return Err(anyhow!("Battery level does not support notifications"));
    }

    let tx_status = tx.clone();
    let connection_status_token = {
        let handler = TypedEventHandler::new(
            move |sender: windows::core::Ref<BluetoothLEDevice>, _args| {
                if let Some(ble) = sender.as_ref() {
                    let status = ble.ConnectionStatus()? == BluetoothConnectionStatus::Connected;
                    let _ = tx_status
                        .try_send(BluetoothLEUpdate::ConnectionStatus(ble_address, status));
                }
                Ok(())
            },
        );
        ble_device.ConnectionStatusChanged(&handler)?
    };

    let tx_battery = tx.clone();
    let battery_token = {
        let handler = TypedEventHandler::new(
            move |_, args: windows::core::Ref<GattValueChangedEventArgs>| {
                if let Ok(args) = args.ok() {
                    let value = args.CharacteristicValue()?;
                    let reader = DataReader::FromBuffer(&value)?;
                    let battery = reader.ReadByte()?;
                    let _ =
                        tx_battery.try_send(BluetoothLEUpdate::BatteryLevel(ble_address, battery));
                }
                Ok(())
            },
        );
        battery_gatt_char.ValueChanged(&handler)?
    };

    Ok((
        ble_device,
        battery_gatt_char,
        connection_status_token,
        battery_token,
    ))
}

struct BatteryState {
    last_update: Instant,
    last_value: u8,
    // Stores a potential new value and when we first saw it.
    pending_state: Option<(u8, Instant)>,
}
const BATTERY_STABILITY_DURATION: Duration = Duration::from_secs(15);
const MINIMUM_UPDATE_INTERVAL: Duration = Duration::from_secs(20);

pub async fn watch_ble_devices_async(
    exit_flag: &Arc<AtomicBool>,
    restart_flag: &Arc<AtomicUsize>,
    proxy: EventLoopProxy<UserEvent>,
) -> Result<()> {
    let mut local_generation = 0;

    let original_ble_devices_address = Arc::new(Mutex::new(HashSet::new()));

    let addresses_to_process: Vec<_> = get_ble_devices_address();

    let ble_devices = futures::stream::iter(addresses_to_process)
        .filter_map(|address| {
            let original_ble_devices_address = original_ble_devices_address.clone();
            async move {
                match get_ble_device_from_address(address).await {
                    Ok(ble_device) => {
                        original_ble_devices_address.lock().await.insert(address);
                        Some((address, ble_device))
                    }
                    Err(_) => None,
                }
            }
        })
        .collect::<Vec<_>>()
        .await;

    let (tx, mut rx) = tokio::sync::mpsc::channel(10);

    let mut guard = scopeguard::guard(HashMap::<u64, WatchBLEGuard>::new(), |map| {
        for (device, char, connection_status_token, battery_token) in map.into_values() {
            let _ = device.RemoveConnectionStatusChanged(connection_status_token);
            let _ = char.RemoveValueChanged(battery_token);
        }
    });

    // 对电量更新进行去抖（Debounce）及节流（Throttle）
    let mut battery_states: HashMap<u64, BatteryState> = HashMap::new();

    for (ble_address, ble_device) in ble_devices {
        let watch_btc_guard = watch_ble_device(ble_address, ble_device, tx.clone()).await?;

        guard.insert(ble_address, watch_btc_guard);
    }

    loop {
        tokio::select! {
            maybe_update= rx.recv() => {
                let Some(update) = maybe_update else {
                    return Err(anyhow!("Channel closed while watching BLE devices"));
                };

                let mut need_update_tray = false;

                match update {
                    BluetoothLEUpdate::BatteryLevel(address, new_battery) => {
                        let Some(mut info) = BT_INFO_MAP.get_mut(&address) else {
                            // 如果在主设备列表中找不到该地址，则跳过
                            continue;
                        };
                        match battery_states.entry(address) {
                            // First time seeing this device
                            Vacant(entry) => {
                                info!("BLE [{}]: Battery -> {new_battery}", info.name);
                                info.battery = new_battery;
                                need_update_tray = true;

                                // Insert its initial state
                                entry.insert(BatteryState {
                                    last_update: Instant::now(),
                                    last_value: new_battery,
                                    pending_state: None,
                                });
                            }
                            Occupied(mut entry) => {
                                let state = entry.get_mut();
                                let mut should_report = false;
                                let mut value_to_report = new_battery;

                                // 逻辑A: 检查数值是否稳定
                                if state.last_value == new_battery {
                                    state.pending_state = None;
                                } else {
                                    match &mut state.pending_state {
                                        Some((pending_value, first_seen_time)) => {
                                            if *pending_value == new_battery {
                                                // 新值和待定值相同，检查是否已稳定足够长的时间
                                                if first_seen_time.elapsed() >= BATTERY_STABILITY_DURATION {
                                                    should_report = true;
                                                }
                                                // else: 时间还不够长，继续等待
                                            } else {
                                                // 值再次跳变，重置待定状态为这个更新的值
                                                info!("BLE [{}]: Battery fluctuated again to {new_battery}, resetting stability check.", info.name);
                                                state.pending_state = Some((new_battery, Instant::now()));
                                            }
                                        },
                                        None => {
                                            info!("BLE [{}]: New potential battery value {new_battery}. Waiting for stability.", info.name);
                                            state.pending_state = Some((new_battery, Instant::now()));
                                        }
                                    }
                                }

                                // 逻辑B: 强制周期性更新 (备用策略)
                                if !should_report
                                    && state.last_update.elapsed() >= MINIMUM_UPDATE_INTERVAL
                                    && state.last_value != new_battery
                                {
                                    should_report = true;
                                    value_to_report = state.pending_state.map_or(new_battery, |(v, _)| v);
                                }

                                if should_report {
                                    info!("BLE [{}]: Battery -> {value_to_report}", info.name);

                                    state.last_value = value_to_report;
                                    state.last_update = Instant::now();
                                    state.pending_state = None; // 成功报告后，清空待定状态

                                    info.battery = value_to_report;
                                    need_update_tray = true;

                                    // 发送通知
                                    let _ = proxy.send_event(UserEvent::Notify(NotifyEvent::LowBattery(
                                        info.name.clone(),
                                        value_to_report,
                                        info.address,
                                    )));
                                }
                            }
                        }
                    }
                    BluetoothLEUpdate::ConnectionStatus(address, status) => {
                        if let Some(mut info) = BT_INFO_MAP.get_mut(&address)
                            && info.status != status {
                                info!("BLE [{}]: Status -> {status}", info.name);
                                info.status = status;
                                need_update_tray = true;
                                let notify_event = if status {
                                    NotifyEvent::Reconnect(info.name.clone())
                                } else {
                                    NotifyEvent::Disconnect(info.name.clone())
                                };
                                let _ = proxy.send_event(UserEvent::Notify(notify_event));
                            }
                    }
                }

                if need_update_tray {
                    let _ = proxy.send_event(UserEvent::UpdateTray);
                }
            },
            _ = async {
                let original_ble_devices_address = Arc::clone(&original_ble_devices_address);

                while !exit_flag.load(Ordering::Relaxed) {
                    let current_generation = restart_flag.load(Ordering::Relaxed);
                    if local_generation < current_generation {
                        info!("Watch BLE restart by restart flag.");
                        local_generation = current_generation;

                        let current_ble_devices_address: HashSet<_> = get_ble_devices_address();

                        let original_ble_devices_address_clone = original_ble_devices_address.lock().await.clone();

                        let removed_devices = original_ble_devices_address_clone
                            .difference(&current_ble_devices_address)
                            .cloned()
                            .collect::<Vec<_>>();

                        let added_devices = current_ble_devices_address
                            .difference(&original_ble_devices_address_clone)
                            .cloned()
                            .collect::<Vec<_>>();

                        for removed_device in removed_devices {
                            guard.remove(&removed_device);
                            original_ble_devices_address.lock().await.remove(&removed_device);
                        }

                        for added_device_address in added_devices {
                            tokio::time::sleep(Duration::from_secs(1)).await;
                            let Ok(ble_device) = get_ble_device_from_address(added_device_address).await else {
                                // 移除错误设备
                                warn!("Failed to get added BLE Device from address");
                                BT_INFO_MAP.remove(&added_device_address);
                                continue;
                            };

                            let name = ble_device.Name().map_or("Unknown name".to_owned(), |n| n.to_string());

                            match watch_ble_device(added_device_address, ble_device, tx.clone()).await  {
                                Ok(watch_ble_guard) => {
                                    guard.insert(added_device_address, watch_ble_guard);
                                    original_ble_devices_address.lock().await.insert(added_device_address);
                                },
                                Err(e) => {
                                    // 移除错误设备
                                    warn!("BLE [{name}]: Failed to watch added BLE Device - {e}");
                                    BT_INFO_MAP.remove(&added_device_address);
                                }
                            }
                        }
                    }

                    tokio::time::sleep(Duration::from_secs(1)).await;
                }

                info!("Watch BLE Battery and Status was cancelled by exit flag.");
            } => return Ok(()),
        }
    }
}
