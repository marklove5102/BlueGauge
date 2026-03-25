pub mod icon;
pub mod menu;

use super::tray::{
    icon::{load_app_icon, load_tray_icon},
    menu::{MenuGroup, item::create_menu},
};
use crate::{
    bluetooth::BT_INFO_MAP,
    config::{CONFIG, TrayIconStyle},
};

use anyhow::{Result, anyhow};
use log::error;
use tray_controls::MenuManager;
use tray_icon::{TrayIcon, TrayIconBuilder};

pub fn create_tray_icon() -> Option<tray_icon::Icon> {
    let tray_icon_bt_address = CONFIG
        .read()
        .unwrap()
        .tray_options
        .tray_icon_style
        .get_address();

    tray_icon_bt_address
        .and_then(|address| BT_INFO_MAP.get(&address))
        .and_then(|entry| {
            load_tray_icon(entry.battery, entry.status)
                .inspect_err(|e| error!("Failed to load icon - {e}"))
                .ok()
        })
        .or_else(|| {
            // 载入图标失败时，需更新配置中的图标样式，注意要在创建菜单之前
            CONFIG.write().unwrap().tray_options.tray_icon_style = TrayIconStyle::App;
            load_app_icon().ok()
        })
}

#[rustfmt::skip]
pub fn create_tray(
    menu_manager: &mut MenuManager<MenuGroup>,
) -> Result<TrayIcon> {
    let icon = create_tray_icon().expect("Failed to create tray's icon");

    let tray_menu =  create_menu(menu_manager)
        .map_err(|e| anyhow!("Failed to create menu. - {e}"))?;

    let bluetooth_tooltip_info = convert_tray_info();

    let tray_icon = TrayIconBuilder::new()
        .with_menu_on_left_click(true)
        .with_icon(icon)
        .with_tooltip(bluetooth_tooltip_info)
        .with_menu(Box::new(tray_menu))
        .build()
        .map_err(|e| anyhow!("Failed to build tray - {e}"))?;

    Ok(tray_icon)
}

/// 返回托盘提示及菜单内容
pub fn convert_tray_info() -> String {
    let config = CONFIG.read().unwrap();
    let should_truncate_name = config.tray_options.tooltip_options.truncate_name();
    let should_prefix_battery = config.tray_options.tooltip_options.prefix_battery();
    let should_show_disconnected = config.tray_options.tooltip_options.show_disconnected();

    let mut sorted_devices_info = BT_INFO_MAP
        .iter()
        .map(|entry| entry.value().clone())
        .collect::<Vec<_>>();

    sorted_devices_info.sort_by(|a, b| {
        // 1. 先按状态排序（🟢在前，🔴在后）
        match (a.status, b.status) {
            (true, false) => std::cmp::Ordering::Less, // true 在 false 前
            (false, true) => std::cmp::Ordering::Greater, // false 在 true 后
            _ => {
                // 2. 同组内按名称字母顺序排序（A-Z）
                a.name.cmp(&b.name)
            }
        }
    });

    sorted_devices_info
        .into_iter()
        .filter_map(|info| {
            let include_in_tooltip = info.status || should_show_disconnected;
            if include_in_tooltip {
                let name = {
                    let name = config.device_aliases.get(&info.name).unwrap_or(&info.name);
                    truncate_with_ellipsis(should_truncate_name, name, 10)
                };
                let battery = info.battery;
                let status_icon = if info.status { "🟢" } else { "🔴" };
                let info = if should_prefix_battery {
                    format!("{status_icon}{battery}% - {name}")
                } else {
                    format!("{status_icon}{name} - {battery}%")
                };
                Some(info)
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn truncate_with_ellipsis(truncate_device_name: bool, name: &str, max_chars: usize) -> String {
    if truncate_device_name && name.chars().count() > max_chars {
        let mut result = name.chars().take(max_chars).collect::<String>();
        result.push('…');
        result
    } else {
        name.to_string()
    }
}
