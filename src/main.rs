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
    convert_tray_info, create_tray, create_tray_icon,
    menu::{
        MenuGroup, about,
        handler::MenuHandler,
        item::{MenuAction, create_menu},
    },
};

use std::ffi::OsString;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::{collections::HashSet, sync::OnceLock};

use log::{error, info};
use tray_controls::MenuManager;
use tray_icon::{TrayIcon, menu::MenuEvent};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    window::WindowId,
};

pub static PROXY: OnceLock<EventLoopProxy<UserEvent>> = OnceLock::new();

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

    PROXY.get_or_init(|| proxy.clone());

    MenuEvent::set_event_handler(Some(move |event| {
        proxy
            .send_event(UserEvent::MenuEvent(event))
            .expect("Failed to send MenuEvent");
    }));

    let mut app = App::new().await;
    event_loop.run_app(&mut app)?;

    Ok(())
}

struct App {
    exit_threads: Arc<AtomicBool>,
    /// 存储已经通知过的低电量设备（地址），避免再次通知
    notified_devices: Arc<Mutex<HashSet</* Address */ u64>>>,
    menu_manager: Mutex<MenuManager<MenuGroup>>,
    system_theme: Arc<RwLock<SystemTheme>>,
    theme_watcher: Option<ThemeWatcher>,
    tray: Mutex<TrayIcon>,
    bluetooth_watcher: Option<Watcher>,
}

impl App {
    async fn new() -> Self {
        {
            Self::send_low_battery_notification();
            Self::handle_show_lowest_battery_device();
        }

        let mut menu_manager = MenuManager::new();

        let tray = create_tray(&mut menu_manager).expect("Failed to create tray");

        Self {
            exit_threads: Arc::new(AtomicBool::new(false)),
            notified_devices: Arc::new(Mutex::new(HashSet::new())),
            menu_manager: Mutex::new(menu_manager),
            system_theme: Arc::new(RwLock::new(SystemTheme::get())),
            theme_watcher: None,
            tray: Mutex::new(tray),
            bluetooth_watcher: None,
        }
    }

    fn send_low_battery_notification() {
        BT_INFO_MAP.iter().for_each(|entry| {
            let _ = PROXY
                .get()
                .unwrap()
                .send_event(UserEvent::Notify(NotifyEvent::LowBattery(
                    entry.name.clone(),
                    entry.battery,
                    entry.address,
                )));
        });
    }

    fn handle_show_lowest_battery_device() {
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

            let tray_icon_style = &mut config.tray_options.tray_icon_style;

            if !tray_icon_style.update_address(*address) {
                *tray_icon_style = TrayIconStyle::number_icon(*address, None);
            }

            config.save_toml();
        }
    }
}

#[derive(Debug)]
pub enum UserEvent {
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
        let mut watch = Watcher::new();
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
        let system_theme = Arc::clone(&self.system_theme);
        let mut theme_watcher = ThemeWatcher::new(exit_threads, system_theme);
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

                    let menu_handlers = MenuHandler::new(menu_control.clone());

                    if let Err(e) = menu_handlers.run() {
                        error!("Failed to handle menu event: {e}")
                    }
                });
            }
            UserEvent::Notify(notify_event) => notify_event.send(self.notified_devices.clone()),
            UserEvent::UpdateTrayIcon => {
                Self::handle_show_lowest_battery_device();

                let Some(icon) = create_tray_icon() else {
                    return;
                };

                let _ = self.tray.lock().unwrap().set_icon(Some(icon));
            }
            UserEvent::UpdateTrayTooltip => {
                let bluetooth_tooltip_info = convert_tray_info();
                let _ = self
                    .tray
                    .lock()
                    .unwrap()
                    .set_tooltip(Some(bluetooth_tooltip_info));
            }
            UserEvent::UpdateTray => {
                // 不创建 UserEvent::HandShowLowestBatteryDevice 事件，是因为 UserEVent 是非同步的，会导致菜单项未得到及时更新
                Self::handle_show_lowest_battery_device();

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

                let proxy = PROXY.get().unwrap();

                let _ = proxy.send_event(UserEvent::UpdateTrayIcon);
                let _ = proxy.send_event(UserEvent::UpdateTrayTooltip);
            }
            UserEvent::Refresh => {
                futures::executor::block_on(async {
                    init_bluetooth_info()
                        .await
                        .expect("Failed to init bt devices info")
                });

                Self::send_low_battery_notification();

                let _ = PROXY.get().unwrap().send_event(UserEvent::UpdateTray);
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

                let _ = PROXY.get().unwrap().send_event(UserEvent::Exit);
            }
            UserEvent::ShowAboutDialog => {
                let hwnd = self.tray.lock().unwrap().window_handle();
                about::show_about_dialog(hwnd as isize);
            }
        }
    }
}
