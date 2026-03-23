#![allow(non_snake_case)]
#![cfg(target_os = "windows")]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bluetooth;
mod config;
mod language;
mod notify;
mod single_instance;
mod startup;
mod theme;
mod tray;
mod util;

use crate::bluetooth::{
    BT_INFO_MAP,
    info::{BluetoothInfo, find_bluetooth_devices, get_bluetooth_devices_info},
    init_bluetooth_info,
    watch::Watcher,
};
use crate::config::{CONFIG, EXE_PATH, TrayIconStyle};
use crate::notify::{NotifyEvent, notify};
use crate::single_instance::SingleInstance;
use crate::theme::{SystemTheme, ThemeWatcher};
use crate::tray::{
    convert_tray_info, create_tray,
    icon::{load_app_icon, load_tray_icon},
    menu::{
        MenuGroup, about,
        handler::MenuHandler,
        item::{MenuAction, create_menu},
    },
};

use std::collections::HashSet;
use std::ffi::OsString;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use log::{error, info};
use tray_controls::MenuManager;
use tray_icon::{TrayIcon, menu::MenuEvent};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    window::WindowId,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _single_instance = SingleInstance::new()?;

    std::panic::set_hook(Box::new(|info| {
        error!("⚠️ Panic: {info}");
        notify(format!("⚠️ Panic: {info}"));
    }));

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    init_bluetooth_info().await?;

    let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;

    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        proxy
            .send_event(UserEvent::MenuEvent(event))
            .expect("Failed to send MenuEvent");
    }));

    let proxy = event_loop.create_proxy();
    let mut app = App::new(proxy).await;
    event_loop.run_app(&mut app)?;

    Ok(())
}

struct App {
    exit_threads: Arc<AtomicBool>,
    event_loop_proxy: EventLoopProxy<UserEvent>,
    /// 存储已经通知过的低电量设备（地址），避免再次通知
    notified_devices: Arc<Mutex<HashSet</* Address */ u64>>>,
    menu_manager: Mutex<MenuManager<MenuGroup>>,
    system_theme: Arc<RwLock<SystemTheme>>,
    theme_watcher: Option<ThemeWatcher>,
    tray: Mutex<TrayIcon>,
    bluetooth_watcher: Option<Watcher>,
}

impl App {
    async fn new(event_loop_proxy: EventLoopProxy<UserEvent>) -> Self {
        let should_show_lowest_battery_device = CONFIG
            .read()
            .unwrap()
            .tray_options
            .show_lowest_battery_device;

        // 首次打开软件时，检测有无低电量及需显示最低电量设备
        {
            let mut should_update_tray_icon_style: Option<(u64, u8)> = None;
            for entry in BT_INFO_MAP.iter() {
                let info = entry.value();
                let _ = event_loop_proxy.send_event(UserEvent::Notify(NotifyEvent::LowBattery(
                    info.name.clone(),
                    info.battery,
                    info.address,
                )));

                if info.status && should_show_lowest_battery_device {
                    match should_update_tray_icon_style {
                        Some((ref mut address, ref mut lowest_battery))
                            if info.battery < *lowest_battery =>
                        {
                            *address = info.address;
                            *lowest_battery = info.battery;
                        }
                        None => {
                            should_update_tray_icon_style = Some((info.address, info.battery));
                        }
                        _ => {}
                    }
                }
            }

            if let Some((address, _)) = should_update_tray_icon_style {
                info!("Show Lowest Battery Device on Startup: {}", address);

                let mut config = CONFIG.write().unwrap();

                if !config.tray_options.tray_icon_style.update_address(address) {
                    // 如果默认是 APP 图标，则切换为数字图标
                    config.tray_options.tray_icon_style =
                        TrayIconStyle::default_number_icon(address, None);
                };

                config.save_toml();
            }
        }

        let mut menu_manager = MenuManager::new();

        let tray = create_tray(&mut menu_manager).expect("Failed to create tray");

        Self {
            event_loop_proxy,
            exit_threads: Arc::new(AtomicBool::new(false)),
            notified_devices: Arc::new(Mutex::new(HashSet::new())),
            menu_manager: Mutex::new(menu_manager),
            system_theme: Arc::new(RwLock::new(SystemTheme::get())),
            theme_watcher: None,
            tray: Mutex::new(tray),
            bluetooth_watcher: None,
        }
    }
}

#[derive(Debug)]
enum UserEvent {
    Exit,
    MenuEvent(MenuEvent),
    Notify(NotifyEvent),
    UnCheckAboutIconMenu,
    UnCheckDeviceMenu,
    UpdateTray,
    UpdateTrayIcon,
    UpdateTrayTooltip,
    Refresh,
    Restart,
    ShowAboutDialog,
}

impl App {
    fn start_watch_devices(&mut self) {
        self.stop_watch_devices();
        let mut watch = Watcher::new(self.event_loop_proxy.clone());
        watch.start();
        self.bluetooth_watcher = Some(watch);
    }

    fn stop_watch_devices(&mut self) {
        if let Some(mut bluetooth_watcher) = self.bluetooth_watcher.take() {
            bluetooth_watcher.stop()
        }
    }

    fn start_watch_theme(&mut self) {
        let exit_threads = Arc::clone(&self.exit_threads);
        let proxy = self.event_loop_proxy.clone();
        let system_theme = Arc::clone(&self.system_theme);
        let mut theme_watcher = ThemeWatcher::new(exit_threads, proxy, system_theme);
        theme_watcher.start();
        self.theme_watcher = Some(theme_watcher);
    }

    fn stop_watch_theme(&mut self) {
        if let Some(mut theme_watcher) = self.theme_watcher.take() {
            theme_watcher.stop()
        }
    }

    fn exit(&mut self) {
        self.exit_threads.store(true, Ordering::Relaxed);
        self.stop_watch_devices();
        self.stop_watch_theme();
    }

    fn handle_show_lowest_battery_device(&mut self) {
        let should_show_lowest_battery_device = CONFIG
            .read()
            .unwrap()
            .tray_options
            .show_lowest_battery_device;

        if should_show_lowest_battery_device
            && let Some(entry) = BT_INFO_MAP
                .iter()
                .filter(|entry| entry.status)
                .min_by_key(|entry| entry.battery)
        {
            let (address, info) = entry.pair();
            info!("Show Lowest Battery Device: {}", info.name);

            let mut config = CONFIG.write().unwrap();

            if !config.tray_options.tray_icon_style.update_address(*address) {
                config.tray_options.tray_icon_style =
                    TrayIconStyle::default_number_icon(*address, None);
            }

            config.save_toml();
        }
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        self.start_watch_devices();
        self.start_watch_theme();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if event == WindowEvent::CloseRequested {
            self.exit();
            event_loop.exit();
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::UnCheckDeviceMenu => {
                if let Some(menu_map) = self
                    .menu_manager
                    .lock()
                    .unwrap()
                    .get_check_items_from_grouped(&MenuGroup::RadioDevice)
                {
                    menu_map.values().for_each(|m| m.set_checked(false));
                }
            }
            // 取消勾选 [显示最低电量设备] 和 [设置连接配色]
            UserEvent::UnCheckAboutIconMenu => {
                if let Some(menu_control) = self
                    .menu_manager
                    .lock()
                    .unwrap()
                    .get_menu_item_from_id(&MenuAction::ShowLowestBatteryDevice.id())
                {
                    menu_control.set_checked(false);
                }

                if let Some(menu_control) = self
                    .menu_manager
                    .lock()
                    .unwrap()
                    .get_menu_item_from_id(&MenuAction::SetIconConnectColor.id())
                {
                    menu_control.set_checked(false);
                }
            }
            UserEvent::Exit => {
                self.exit();
                event_loop.exit();
            }
            UserEvent::MenuEvent(event) => {
                let mut menu_manager = self.menu_manager.lock().unwrap();
                menu_manager.update(event.id(), |menu_control| {
                    let Some(menu_control) = menu_control else {
                        error!("Failed to get menu control");
                        return;
                    };

                    let menu_handlers =
                        MenuHandler::new(menu_control.clone(), self.event_loop_proxy.clone());

                    if let Err(e) = menu_handlers.run() {
                        error!("Failed to handle menu event: {e}")
                    }
                });
            }
            UserEvent::Notify(notify_event) => notify_event.send(self.notified_devices.clone()),
            UserEvent::UpdateTrayIcon => {
                self.handle_show_lowest_battery_device();

                let tray_icon_bt_address = CONFIG
                    .read()
                    .unwrap()
                    .tray_options
                    .tray_icon_style
                    .get_address();

                let icon = tray_icon_bt_address
                    .and_then(|address| BT_INFO_MAP.get(&address))
                    .and_then(|info| {
                        load_tray_icon(info.battery, info.status)
                            .inspect_err(|e| error!("Failed to load icon - {e}"))
                            .ok()
                    })
                    .or_else(|| {
                        // 载入图标失败时，需更新配置中的图标样式，注意要在创建菜单之前
                        CONFIG.write().unwrap().tray_options.tray_icon_style = TrayIconStyle::App;
                        load_app_icon().ok()
                    });

                let _ = self.tray.lock().unwrap().set_icon(icon);
            }
            UserEvent::UpdateTrayTooltip => {
                let bluetooth_tooltip_info = convert_tray_info();
                let _ = self
                    .tray
                    .lock()
                    .unwrap()
                    .set_tooltip(Some(bluetooth_tooltip_info.join("\n")));
            }
            UserEvent::UpdateTray => {
                // 不创建 UserEvent::HandShowLowestBatteryDevice 事件，是因为 UserEVent 是非同步的，会导致菜单项未得到及时更新
                self.handle_show_lowest_battery_device();

                let tray_menu = {
                    let mut menu_manager = self.menu_manager.lock().unwrap();
                    match create_menu(&mut menu_manager) {
                        Ok(tray_menu) => tray_menu,
                        Err(e) => {
                            notify(format!("Failed to create tray menu - {e}"));
                            return;
                        }
                    }
                };

                // UserEvent发送的事件是异步的，如果在UpdateTrayIcon在创建菜单前，Handle显示最低电量设备可能不及时导致菜单设备项未得到及时更新
                self.tray
                    .lock()
                    .unwrap()
                    .set_menu(Some(Box::new(tray_menu)));
                let _ = self.event_loop_proxy.send_event(UserEvent::UpdateTrayIcon);
                let _ = self
                    .event_loop_proxy
                    .send_event(UserEvent::UpdateTrayTooltip);
            }
            UserEvent::Refresh => {
                futures::executor::block_on(async {
                    init_bluetooth_info()
                        .await
                        .expect("Failed to init bt devices info")
                });

                for entry in BT_INFO_MAP.iter() {
                    let info = entry.value();
                    let _ = self.event_loop_proxy.send_event(UserEvent::Notify(
                        NotifyEvent::LowBattery(info.name.clone(), info.battery, info.address),
                    ));
                }

                let _ = self.event_loop_proxy.send_event(UserEvent::UpdateTray);
            }
            UserEvent::Restart => {
                let mut args_os: Vec<OsString> = std::env::args_os().collect();
                args_os.push("--restart".into()); // 添加重启标志（避免与单实例冲突）

                if let Err(e) = Command::new(&*EXE_PATH)
                    .args(args_os.iter().skip(1))
                    .spawn()
                {
                    notify(format!("Failed to restart app: {e}"));
                }

                let _ = self.event_loop_proxy.send_event(UserEvent::Exit);
            }
            UserEvent::ShowAboutDialog => {
                let hwnd = self.tray.lock().unwrap().window_handle();
                about::show_about_dialog(hwnd as isize);
            }
        }
    }
}
