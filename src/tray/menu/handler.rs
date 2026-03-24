use super::{MenuGroup, item::*};
use crate::{
    PROXY, UserEvent,
    config::{CONFIG, CONFIG_PATH, TrayIconStyle},
    startup::set_startup,
};

use std::{process::Command, str::FromStr};

use anyhow::{Context, Result, anyhow};
use tray_controls::{CheckMenuKind, MenuControl};

pub struct MenuHandler<MenuGroup> {
    menu_control: MenuControl<MenuGroup>,
}

impl MenuHandler<MenuGroup> {
    pub fn new(menu_control: MenuControl<MenuGroup>) -> Self {
        Self { menu_control }
    }

    pub fn run(&self) -> Result<()> {
        let menu_control = &self.menu_control;

        let id = self.menu_control.id();

        let menu_action = match MenuAction::from_str(id.as_ref()) {
            Ok(m) => m,
            Err(e) => {
                if matches!(
                    menu_control,
                    MenuControl::CheckMenu(CheckMenuKind::Radio(_, _, MenuGroup::RadioDevice))
                ) {
                    MenuAction::Device
                } else {
                    return Err(anyhow!("No match check menu [{}] - {e}", id.0));
                }
            }
        };

        let mut config = CONFIG.write().unwrap();

        let proxy = PROXY.lock().unwrap().clone().unwrap();

        match menu_control {
            MenuControl::CheckMenu(check_menu_kind) => match check_menu_kind {
                CheckMenuKind::Separate(check_menu) => {
                    let check_state = check_menu.is_checked();

                    match menu_action {
                        MenuAction::Startup => set_startup(check_state),
                        MenuAction::ShowLowestBatteryDevice => {
                            config
                                .tray_options
                                .set_show_lowest_battery_device(check_state);

                            config.save_toml();

                            drop(config);

                            proxy
                                .send_event(UserEvent::UpdateTray)
                                .context("Failed to send 'Update Tray' event")
                        }
                        MenuAction::SetIconConnectColor => {
                            config
                                .tray_options
                                .tray_icon_style
                                .set_connect_color(check_state);

                            config.save_toml();

                            drop(config);

                            proxy
                                .send_event(UserEvent::UpdateTrayIcon)
                                .context("Failed to send 'Update Tray Icon' event")
                        }
                        _ => Err(anyhow!("No match single check menu: {}", id.0)),
                    }
                }
                CheckMenuKind::CheckBox(check_menu, group) => match group {
                    MenuGroup::CheckBoxNotify => {
                        let check_state = check_menu.is_checked();
                        let notify_options = &mut config.notify_options;

                        match menu_action {
                            MenuAction::NotifyDeviceChangeDisconnection => {
                                notify_options.set_disconnection(check_state);
                            }
                            MenuAction::NotifyDeviceChangeReconnection => {
                                notify_options.set_reconnection(check_state);
                            }
                            MenuAction::NotifyDeviceChangeAdded => {
                                notify_options.set_added(check_state);
                            }
                            MenuAction::NotifyDeviceChangeRemoved => {
                                notify_options.set_removed(check_state);
                            }
                            MenuAction::NotifyDeviceStayOnScreen => {
                                notify_options.set_stay_on_screen(check_state);
                            }
                            _ => return Err(anyhow!("No match set notify menu: {}", id.0)),
                        }

                        config.save_toml();

                        Ok(())
                    }
                    MenuGroup::CheckBoxTrayTooltip => {
                        let check_state = check_menu.is_checked();
                        let tooltip_options = &mut config.tray_options.tooltip_options;

                        match menu_action {
                            MenuAction::TrayTooltipShowDisconnected => {
                                tooltip_options.set_show_disconnected(check_state);
                            }
                            MenuAction::TrayTooltipTruncateName => {
                                tooltip_options.set_truncate_name(check_state);
                            }
                            MenuAction::TrayTooltipPrefixBattery => {
                                tooltip_options.set_prefix_battery(check_state);
                            }
                            _ => return Err(anyhow!("No match set tray tooltip menu: {}", id.0)),
                        }

                        config.save_toml();

                        drop(config);

                        proxy
                            .send_event(UserEvent::UpdateTrayTooltip)
                            .context("Failed to send 'Update Tray' event")
                    }
                    _ => Err(anyhow!("Not support check menu group: {}", id.0)),
                },
                CheckMenuKind::Radio(check_menu, _, group) => {
                    match group {
                        MenuGroup::RadioDevice => {
                            let tray_icon_style = &config.tray_options.tray_icon_style;

                            if check_menu.is_checked() {
                                let device_menu_id = check_menu.id();
                                let device_address =
                                    device_menu_id.as_ref().parse::<u64>().unwrap_or_else(|_| {
                                        panic!("The menu isn't device menu: {}", device_menu_id.0)
                                    });
                                if matches!(tray_icon_style, TrayIconStyle::App) {
                                    config.tray_options.tray_icon_style =
                                        TrayIconStyle::default_number_icon(device_address, None);
                                } else {
                                    config
                                        .tray_options
                                        .tray_icon_style
                                        .update_address(device_address);
                                }
                            } else {
                                // 全部设备未勾选，设置图标样式变回 AppIcon
                                config.tray_options.tray_icon_style = TrayIconStyle::App;
                                config.tray_options.set_show_lowest_battery_device(false);
                                let _ = proxy.send_event(UserEvent::UnCheckAboutIconMenu);
                            }

                            config.save_toml();

                            drop(config);

                            proxy
                                .send_event(UserEvent::UpdateTray)
                                .context("Failed to send 'Update Icon' event")
                        }
                        MenuGroup::RadioLowBattery => {
                            let low_battery = check_menu.id().as_ref().parse::<u8>()?;
                            let should_notify = low_battery.ne(&0);

                            config.notify_options.low_battery.set_notify(should_notify);

                            if should_notify {
                                config.notify_options.low_battery.set_value(low_battery);
                            };

                            config.save_toml();

                            drop(config);

                            // 更新托盘是因为某些设备低于
                            proxy
                                .send_event(UserEvent::UpdateTrayIcon)
                                .context("Failed to send 'Update Tray' event")
                        }
                        MenuGroup::RadioTrayIconStyle => {
                            let select_menu_id = check_menu.id();
                            let Ok(select_menu_action) =
                                MenuAction::from_str(select_menu_id.as_ref())
                            else {
                                return Err(anyhow!(
                                    "No match set tray icon style menu: {}",
                                    select_menu_id.0
                                ));
                            };

                            let tray_icon_style = &config.tray_options.tray_icon_style;

                            let Some(address) = tray_icon_style.get_address() else {
                                // 若为App图标，即为无勾选设备，则返回
                                return Ok(());
                            };

                            let color_scheme = tray_icon_style.get_color_scheme();

                            match select_menu_action {
                                MenuAction::TrayIconStyleApp => {
                                    // 取消勾选所有设备菜单，取消显示最低电量设备选项
                                    config.tray_options.set_show_lowest_battery_device(false);
                                    let _ = proxy.send_event(UserEvent::UnCheckDeviceMenu);
                                    let _ = proxy.send_event(UserEvent::UnCheckAboutIconMenu);
                                    config.tray_options.tray_icon_style = TrayIconStyle::App;
                                }
                                MenuAction::TrayIconStyleHorizontalBattery => {
                                    config.tray_options.tray_icon_style =
                                        TrayIconStyle::default_hor_battery_icon(
                                            address,
                                            color_scheme,
                                        )
                                }
                                MenuAction::TrayIconStyleVerticalBattery => {
                                    config.tray_options.tray_icon_style =
                                        TrayIconStyle::default_vrt_battery_icon(
                                            address,
                                            color_scheme,
                                        )
                                }
                                MenuAction::TrayIconStyleNumber => {
                                    config.tray_options.tray_icon_style =
                                        TrayIconStyle::default_number_icon(address, color_scheme)
                                }
                                MenuAction::TrayIconStyleRing => {
                                    config.tray_options.tray_icon_style =
                                        TrayIconStyle::default_ring_icon(address, color_scheme)
                                }
                                _ => {
                                    return Err(anyhow!(
                                        "No match set tray icon style menu: {}",
                                        id.0
                                    ));
                                }
                            }

                            config.save_toml();

                            drop(config);

                            proxy
                                .send_event(UserEvent::UpdateTrayIcon)
                                .context("Failed to send 'Update Tray' event")
                        }
                        _ => Err(anyhow!("Not support check menu group: {}", id.0)),
                    }
                }
            },
            MenuControl::IconMenu(_icon_menu) => Err(anyhow!("None icon menu")),
            MenuControl::MenuItem(menu_item) => {
                let Ok(menu_action) = MenuAction::from_str(menu_item.id().as_ref()) else {
                    return Err(anyhow!("No match check menu: {}", id.0));
                };

                match menu_action {
                    MenuAction::Quit => proxy
                        .send_event(UserEvent::Exit)
                        .context("Failed to send 'Exit' event"),
                    MenuAction::About => proxy
                        .send_event(UserEvent::ShowAboutDialog)
                        .context("Failed to send 'Show About Dialog' event"),
                    MenuAction::Refresh => proxy
                        .send_event(UserEvent::Refresh)
                        .context("Failed to send 'Refresh' event"),
                    MenuAction::Restart => proxy
                        .send_event(UserEvent::Restart)
                        .context("Failed to send 'Restart' event"),
                    MenuAction::OpenConfig => Command::new("notepad.exe")
                        .arg(&*CONFIG_PATH)
                        .spawn()
                        .map(|_| ())
                        .context("Failed to open config file"),
                    _ => Err(anyhow!("No match normal menu: {}", id.0)),
                }
            }
        }
    }
}
