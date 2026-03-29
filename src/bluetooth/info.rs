use crate::{
    bluetooth::{
        ble::{find_ble_devices, get_ble_devices_info},
        btc::{find_btc_devices, get_btc_devices_info},
    },
    notify::notify,
};

use anyhow::{Result, anyhow};
use dashmap::DashMap;
use log::{info, warn};
use strum::EnumIs;
use windows::Devices::Bluetooth::{BluetoothDevice, BluetoothLEDevice};

#[derive(Default, Clone, PartialEq, Eq, Hash, Debug, EnumIs)]
pub enum BluetoothType {
    Classic(/* Instance ID */ String),
    #[default]
    LowEnergy,
}

#[derive(Default, Clone, PartialEq, Eq, Hash, Debug)]
pub struct BluetoothInfo {
    pub name: String,
    pub battery: u8,
    pub status: bool,
    pub address: u64,
    pub r#type: BluetoothType,
}

impl BluetoothInfo {
    pub fn get_btc_instance_id(&self) -> Option<String> {
        if let BluetoothType::Classic(id) = &self.r#type {
            Some(id.clone())
        } else {
            None
        }
    }

    pub fn is_btc(&self) -> bool {
        self.r#type.is_classic()
    }

    pub fn is_ble(&self) -> bool {
        self.r#type.is_low_energy()
    }
}

pub async fn find_bluetooth_devices() -> Result<(Vec<BluetoothDevice>, Vec<BluetoothLEDevice>)> {
    let bt_devices_futrue = find_btc_devices();
    let ble_devices_futrue = find_ble_devices();

    let (bt_devices, ble_devices) = tokio::join!(bt_devices_futrue, ble_devices_futrue);
    Ok((bt_devices?, ble_devices?))
}

pub async fn get_bluetooth_devices_info(
    bt_devices: (&[BluetoothDevice], &[BluetoothLEDevice]),
) -> Result<DashMap<u64, BluetoothInfo>> {
    let btc_devices = bt_devices.0;
    let ble_devices = bt_devices.1;
    match (btc_devices.len(), ble_devices.len()) {
        (0, 0) => Err(anyhow!("No BTC and BLE devices found")),
        (0, _) => {
            let ble_devices_result = get_ble_devices_info(ble_devices).await;
            info!("{ble_devices_result:#?}");

            ble_devices_result.or_else(|e| {
                notify(format!("Warning: Failed to get BLE devices info: {e}"));
                Ok(DashMap::new())
            })
        }
        (_, 0) => {
            let btc_devices_result = get_btc_devices_info(btc_devices).await;
            info!("{btc_devices_result:#?}");

            btc_devices_result.or_else(|e| {
                notify(format!("Warning: Failed to get BTC devices info: {e}"));
                Ok(DashMap::new())
            })
        }
        (_, _) => {
            let btc_future = get_btc_devices_info(btc_devices);
            let ble_future = get_ble_devices_info(ble_devices);

            let (btc_result, ble_result) = tokio::join!(btc_future, ble_future);

            info!("{btc_result:#?}");
            info!("{ble_result:#?}");

            match (btc_result, ble_result) {
                (Ok(btc_info), Ok(ble_info)) => {
                    let combined_info = btc_info.into_iter().chain(ble_info).collect();
                    Ok(combined_info)
                }
                (Ok(btc_info), Err(e)) => {
                    warn!("Failed to get BLE devices info: {e}");
                    Ok(btc_info)
                }
                (Err(e), Ok(ble_info)) => {
                    warn!("Failed to get BTC devices info: {e}");
                    Ok(ble_info)
                }
                (Err(btc_err), Err(ble_err)) => Err(anyhow!(
                    "Failed to get both BTC and BLE info: {btc_err} | {ble_err}"
                )),
            }
        }
    }
}
