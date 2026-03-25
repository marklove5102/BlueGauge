use super::{
    BT_INFO_MAP,
    ble::{process_ble_device, watch_ble_devices_async},
    btc::{
        get_btc_info_device_frome_address, watch_btc_devices_battery,
        watch_btc_devices_status_async,
    },
    info::BluetoothInfo,
};
use crate::{PROXY, UserEvent, notify::NotifyEvent};

use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use anyhow::{Context, Result, anyhow};
use dashmap::Entry;
use log::{info, warn};
use tokio::{sync::mpsc::Sender, task::JoinHandle};
use windows::{
    Devices::{
        Bluetooth::{BluetoothConnectionStatus, BluetoothDevice, BluetoothLEDevice},
        Enumeration::{
            DeviceInformation, DeviceInformationUpdate, DeviceWatcher, DeviceWatcherStatus,
        },
    },
    Foundation::TypedEventHandler,
    core::{HSTRING, Ref},
};

type WatchHandle = JoinHandle<Result<(), anyhow::Error>>;

macro_rules! spawn_watch {
    ($func:expr, $exit_flag:expr, $restart_flag:expr) => {{
        let exit_flag = Arc::clone(&$exit_flag);
        let restart_flag = Arc::clone(&$restart_flag);

        tokio::spawn(async move { $func(&exit_flag, &restart_flag).await })
    }};
}

pub struct Watcher {
    watch_handles: Option<[WatchHandle; 4]>,
    exit_flag: Arc<AtomicBool>,
    restart_flag: Arc<AtomicUsize>,
}

impl Watcher {
    pub fn new() -> Self {
        let exit_flag = Arc::new(AtomicBool::new(false));
        let restart_flag = Arc::new(AtomicUsize::new(0));
        Self {
            watch_handles: None,
            exit_flag,
            restart_flag,
        }
    }

    pub fn start(&mut self) {
        info!("Starting the watch bluetooth thread...");
        let watch_handles = self.watch_loop();
        self.watch_handles = Some(watch_handles);
    }

    pub fn stop(&mut self) {
        info!("Stopping the watch bluetooth thread...");
        self.exit_flag.store(true, Ordering::Relaxed);
        self.restart_flag.store(0, Ordering::Relaxed);
        self.watch_handles
            .take()
            .iter()
            .flatten()
            .for_each(|h| h.abort());
    }

    #[rustfmt::skip]
    fn watch_loop(&self) -> [WatchHandle; 4] {
        info!("The watch bluetooth thread is started.");

        let watch_btc_battery_handle = spawn_watch!(watch_btc_devices_battery, self.exit_flag, self.restart_flag);
        let watch_btc_status_handle = spawn_watch!(watch_btc_devices_status_async, self.exit_flag, self.restart_flag);
        let watch_ble_handle = spawn_watch!(watch_ble_devices_async, self.exit_flag, self.restart_flag);
        let watch_bt_presence_handle = spawn_watch!(watch_bt_presence_async, self.exit_flag, self.restart_flag);

        [
            watch_ble_handle,
            watch_btc_battery_handle,
            watch_btc_status_handle,
            watch_bt_presence_handle,
        ]
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(PartialEq, Eq)]
enum BluetoothPresence {
    Added,
    Removed,
}

async fn check_presence_async(
    is_ble: bool,
    presence: BluetoothPresence,
    id: HSTRING,
    tx: Sender<(BluetoothInfo, BluetoothPresence)>,
) -> Result<()> {
    match presence {
        BluetoothPresence::Added => {
            if is_ble {
                let ble_device = BluetoothLEDevice::FromIdAsync(&id)?.await?;
                match process_ble_device(&ble_device).await {
                    Ok(ble_info) => {
                        let _ = tx.send((ble_info, presence)).await;
                    }
                    Err(e) => {
                        let name = match ble_device.Name() {
                            Ok(name) if !name.is_empty() => name.to_string(),
                            _ => "Unknown name".to_owned(),
                        };
                        let error = format!("BLE [{name}]: Failed to get info: {e}");
                        return Err(anyhow!(error));
                    }
                }
            } else {
                let btc_device = BluetoothDevice::FromIdAsync(&id)?.await?;
                let process_btc_device = async |btc_device: &BluetoothDevice| {
                    let btc_name = btc_device.Name()?.to_string();
                    let btc_address = btc_device.BluetoothAddress()?;
                    let btc_status =
                        btc_device.ConnectionStatus()? == BluetoothConnectionStatus::Connected;
                    // NOTE: 等待Pnp设备初始化后方可获取经典蓝牙信息
                    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                    get_btc_info_device_frome_address(btc_name.clone(), btc_address, btc_status)
                        .await
                };
                match process_btc_device(&btc_device).await {
                    Ok(btc_info) => {
                        let _ = tx.send((btc_info, presence)).await;
                    }
                    Err(e) => {
                        let name = match btc_device.Name() {
                            Ok(name) if !name.is_empty() => name.to_string(),
                            _ => "Unknown name".to_owned(),
                        };
                        let error = format!("BTC [{name}]: Failed to get info: {e}");
                        return Err(anyhow!(error));
                    }
                }
            };
        }
        BluetoothPresence::Removed => {
            let remove_device_address = if is_ble {
                let device = BluetoothLEDevice::FromIdAsync(&id)?.await?;
                device.BluetoothAddress()?
            } else {
                let device = BluetoothDevice::FromIdAsync(&id)?.await?;
                device.BluetoothAddress()?
            };
            let remove_device_info = BluetoothInfo {
                address: remove_device_address,
                ..Default::default()
            };

            let _ = tx.send((remove_device_info, presence)).await;
        }
    }

    Ok(())
}

macro_rules! create_presence_handler {
    ($tx:ident, $current_runtime:expr, $arg_type:ty, $is_ble:expr, $presence:expr) => {{
        let handler_tx = $tx.clone();
        TypedEventHandler::new(
            move |_watcher: Ref<DeviceWatcher>, event_info: Ref<$arg_type>| {
                if let Some(info) = event_info.as_ref() {
                    let id = info.Id()?;

                    let result = match $current_runtime.as_ref() {
                        Some(handle) => handle.block_on(async {
                            check_presence_async($is_ble, $presence, id, handler_tx.clone()).await
                        }),
                        None => {
                            // 没有当前 Runtime，创建临时单线程 Runtime
                            let rt = tokio::runtime::Builder::new_current_thread()
                                .enable_all()
                                .build()
                                .map_err(|e| -> windows::core::Error { e.into() })?;

                            rt.block_on(async {
                                check_presence_async($is_ble, $presence, id, handler_tx.clone())
                                    .await
                            })
                        }
                    };

                    result.map_err(|e| {
                        warn!("Failed to create Presence Hnalder: {e}");
                        windows::core::Error::new(
                            windows::core::HRESULT(0x80004005u32 as i32), // E_FAIL
                            e.to_string(),
                        )
                    })?;
                }
                Ok(())
            },
        )
    }};
}

fn start_bt_presence_watch(device_watcher: &DeviceWatcher) -> Result<()> {
    let status = device_watcher.Status()?;

    if matches!(
        status,
        DeviceWatcherStatus::Aborted | DeviceWatcherStatus::Created | DeviceWatcherStatus::Stopped
    ) {
        device_watcher
            .Start()
            .with_context(|| "Failed to start watch for the DeviceWatcher")
    } else {
        Err(anyhow!(
            "DeviceWatcher is already started or starting, current status: {status:?}"
        ))
    }
}

fn stop_bt_presence_watch(device_watcher: &DeviceWatcher) -> Result<()> {
    let status = device_watcher.Status()?;

    if matches!(
        status,
        DeviceWatcherStatus::Aborted
            | DeviceWatcherStatus::EnumerationCompleted
            | DeviceWatcherStatus::Started
    ) {
        device_watcher
            .Stop()
            .with_context(|| "Failed to stop watch for the DeviceWatcher")
    } else {
        Err(anyhow!(
            "DeviceWatcher is already stoped or stoping, current status: {status:?}"
        ))
    }
}

#[rustfmt::skip]
async fn watch_bt_presence_async(
    exit_flag: &Arc<AtomicBool>,
    restart_flag: &Arc<AtomicUsize>,
) -> Result<()> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(10);

    let current_runtime = tokio::runtime::Handle::try_current().ok();

    let btc_filter = BluetoothDevice::GetDeviceSelector()?;
    let btc_watcher = DeviceInformation::CreateWatcherAqsFilter(&btc_filter)?;
    let btc_tokens = {
        let rt_added = current_runtime.clone();
        let rt_removed = current_runtime.clone();
        let added_handler = create_presence_handler!(tx, rt_added, DeviceInformation, false, BluetoothPresence::Added);
        let removed_handler = create_presence_handler!(tx, rt_removed, DeviceInformationUpdate, false, BluetoothPresence::Removed);
        let btc_watch_added_token = btc_watcher.Added(&added_handler)?;
        let btc_watch_removed_token = btc_watcher.Removed(&removed_handler)?;
        [btc_watch_added_token, btc_watch_removed_token]
    };

    let ble_filter = BluetoothLEDevice::GetDeviceSelector()?;
    let ble_watcher = DeviceInformation::CreateWatcherAqsFilter(&ble_filter)?;
    let ble_tokens = {
        let rt_added = current_runtime.clone();
        let rt_removed = current_runtime.clone();
        let added_handler = create_presence_handler!(tx, rt_added, DeviceInformation, true, BluetoothPresence::Added);
        let removed_handler = create_presence_handler!(tx, rt_removed, DeviceInformationUpdate, true, BluetoothPresence::Removed);
        let ble_watch_added_token = ble_watcher.Added(&added_handler)?;
        let ble_watch_removed_token = ble_watcher.Removed(&removed_handler)?;
        [ble_watch_added_token, ble_watch_removed_token]
    };

    start_bt_presence_watch(&btc_watcher)?;
    start_bt_presence_watch(&ble_watcher)?;

    scopeguard::defer! {
        btc_tokens.into_iter().enumerate().for_each(|(index, token)| match index {
            0 => { let _ = btc_watcher.RemoveAdded(token); },
            1 => { let _ = btc_watcher.RemoveRemoved(token); },
            _ => ()
        });
        ble_tokens.into_iter().enumerate().for_each(|(index, token)| match index {
            0 => { let _ = ble_watcher.RemoveAdded(token); },
            1 => { let _ = ble_watcher.RemoveRemoved(token); },
            _ => ()
        });

        stop_bt_presence_watch(&btc_watcher).unwrap();
        stop_bt_presence_watch(&ble_watcher).unwrap();
    }

    let proxy = PROXY.get().unwrap();

    loop {
        tokio::select! {
            maybe_update = rx.recv() => {
                let Some((info, presence)) = maybe_update else {
                    return Err(anyhow!("Channel closed while watching Bluetooth presence"));
                };

                let update_event = |presence: BluetoothPresence, name: String| {
                    // 设备添加/移除后，所有监听增加或移除设备
                    restart_flag.fetch_add(1, Ordering::Relaxed);
                    // 更新托盘信息
                    let _ = proxy.send_event(UserEvent::UpdateTray);
                    // 因 Watcher 无 Config，需传递给有通知配置的 APP 结构体
                    match presence {
                        BluetoothPresence::Added => {
                            info!("[{name}]: New Bluetooth Device Connected");
                            let _ = proxy.send_event(UserEvent::Notify(NotifyEvent::Added(name)));
                        }
                        BluetoothPresence::Removed => {
                            info!("[{name}]: Bluetooth Device Removed");
                            let _ = proxy.send_event(UserEvent::Notify(NotifyEvent::Removed(name)));
                        }
                    }
                };

                if let Entry::Vacant(e) = BT_INFO_MAP.entry(info.address) {
                    match presence {
                        BluetoothPresence::Removed => (), // 原设备无该设备，且该设备实际不存电量服务但可获取得到该服务
                        BluetoothPresence::Added => {
                            let name = info.name.clone();
                            e.insert(info);
                            update_event(presence, name);
                        }
                    }
                } else {
                    match presence {
                        BluetoothPresence::Added => (), // 原设备未被移除
                        BluetoothPresence::Removed => {
                            let removed_info = BT_INFO_MAP.remove(&info.address);
                            let name = match removed_info {
                                Some((_, i)) if !i.name.is_empty() => i.name,
                                _ => "Unknown name".to_owned(),
                            };
                            update_event(presence, name);
                        }
                    }
                }
            }
            _ = async {
                while !exit_flag.load(Ordering::Relaxed) {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            } => {
                info!("Watch Bluetooth Presence was cancelled by exit flag.");
                return Ok(());
            },
        }
    }
}
