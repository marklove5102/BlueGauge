pub mod ble;
pub mod btc;
pub mod info;
pub mod watch;

use super::{BluetoothInfo, find_bluetooth_devices, get_bluetooth_devices_info};

use std::sync::LazyLock;

use anyhow::Result;
use dashmap::DashMap;

pub static BT_INFO_MAP: LazyLock<DashMap<u64, BluetoothInfo>> = LazyLock::new(DashMap::new);

pub async fn init_bluetooth_info() -> Result<()> {
    BT_INFO_MAP.clear();

    let (btc_devices, ble_devices) = find_bluetooth_devices().await?;
    let info = get_bluetooth_devices_info((&btc_devices, &ble_devices)).await?;

    for (addr, i) in info {
        BT_INFO_MAP.insert(addr, i);
    }

    Ok(())
}
