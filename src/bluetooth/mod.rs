pub mod ble;
pub mod btc;
pub mod info;
pub mod watch;

use crate::config::CONFIG;

use super::{BluetoothInfo, find_bluetooth_devices, get_bluetooth_devices_info};

use std::sync::LazyLock;

use anyhow::Result;
use dashmap::DashMap;

pub static BT_INFO_MAP: LazyLock<DashMap<u64, BluetoothInfo>> = LazyLock::new(DashMap::new);

pub async fn init_bluetooth_info() -> Result<()> {
    let (btc_devices, ble_devices) = find_bluetooth_devices().await?;
    let bt_devices_info = get_bluetooth_devices_info((&btc_devices, &ble_devices)).await?;

    let mut config = CONFIG.write().unwrap();

    BT_INFO_MAP.clear();

    for (addr, i) in bt_devices_info {
        let name = i.name.clone();
        BT_INFO_MAP.insert(addr, i);
        config.device_aliases.entry(name.clone()).or_insert(name);
    }

    config.save_toml();

    Ok(())
}
