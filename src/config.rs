use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, RwLock};

use anyhow::{Result, anyhow};
use getset::{CopyGetters, Getters, Setters};
use log::warn;
use piet_common::Color;
use serde::{Deserialize, Serialize};

pub static ASSETS_PATH: LazyLock<PathBuf> = LazyLock::new(|| EXE_PATH.with_file_name("assets"));

pub static CONFIG: LazyLock<RwLock<Config>> =
    LazyLock::new(|| RwLock::new(Config::open().expect("Failed to open config")));

pub static CONFIG_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| EXE_PATH.with_file_name("BlueGauge.toml"));

pub static EXE_NAME: LazyLock<String> = LazyLock::new(|| {
    Path::new(&*EXE_PATH)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| stem.to_owned())
        .expect("Failed to get EXE name")
});

pub static EXE_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| std::env::current_exe().expect("Failed to get BlueGauge.exe path"));

pub static EXE_PATH_STRING: LazyLock<String> = LazyLock::new(|| {
    EXE_PATH
        .to_str()
        .map(|s| s.to_string())
        .expect("Failed to convert EXE 'Path' to 'String'")
});

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(rename = "tray")]
    pub tray_options: TrayOptions,
    #[serde(rename = "notify")]
    pub notify_options: NotifyOptions,
    pub device_aliases: HashMap<String, String>,
}

impl Default for Config {
    fn default() -> Self {
        let device_aliases =
            HashMap::from([("e.g. WH-1000XM6".to_owned(), "Sony Headphones".to_owned())]);

        Self {
            tray_options: Default::default(),
            notify_options: Default::default(),
            device_aliases,
        }
    }
}

impl Config {
    pub fn open() -> Result<Self> {
        let default_config = Config::default();

        Config::read_toml(&CONFIG_PATH).or_else(|e| {
            warn!("Failed to read the config file: {e}\nNow creat a new config file");
            let toml_str = toml::to_string_pretty(&default_config)?;
            std::fs::write(&*CONFIG_PATH, toml_str)?;
            Ok(default_config)
        })
    }

    pub fn save_toml(&self) {
        let toml_str = toml::to_string_pretty(self)
            .expect("Failed to serialize ConfigToml structure as a String of TOML.");
        std::fs::write(&*CONFIG_PATH, toml_str)
            .expect("Failed to write TOML String to BlueGauge.toml");
    }

    fn read_toml(config_path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(config_path)?;
        let mut toml_config: Config = toml::from_str(&content)?;

        if find_custom_icon().is_ok() {
            let tray_icon_style = toml_config.tray_options.tray_icon_style;
            toml_config.tray_options.tray_icon_style = tray_icon_style
                .get_address()
                .map_or(TrayIconStyle::App, |address| TrayIconStyle::BatteryCustom {
                    address,
                });
        } else {
            match toml_config.tray_options.tray_icon_style {
                TrayIconStyle::BatteryNumber {
                    ref mut color_scheme,
                    ref font_color,
                    ..
                } => {
                    if font_color
                        .as_ref()
                        .is_some_and(|c| Color::from_hex_str(c).is_ok())
                    {
                        color_scheme.set_custom();
                    } else if color_scheme.is_custom() {
                        // 如果颜色不存在或错误，且设置自定义，则更改为跟随系统主题
                        color_scheme.set_follow_system_theme();
                    }
                }
                TrayIconStyle::BatteryRing {
                    ref mut color_scheme,
                    ref highlight_color,
                    ref background_color,
                    ..
                } => {
                    let has_valid_custom_color = highlight_color
                        .as_ref()
                        .is_some_and(|c| Color::from_hex_str(c).is_ok())
                        || background_color
                            .as_ref()
                            .is_some_and(|c| Color::from_hex_str(c).is_ok());

                    if has_valid_custom_color {
                        color_scheme.set_custom();
                    } else if color_scheme.is_custom() {
                        // 如果颜色不存在或错误，且设置自定义，则更改为跟随系统主题
                        color_scheme.set_follow_system_theme();
                    }
                }
                _ => (),
            }
        };

        Ok(toml_config)
    }
}

impl Config {
    pub fn get_tray_battery_icon_bt_address(&self) -> Option<u64> {
        let tray_icon_style = &self.tray_options.tray_icon_style;

        match tray_icon_style {
            TrayIconStyle::App => None,
            TrayIconStyle::BatteryCustom { address } => Some(*address),
            TrayIconStyle::BatteryIcon { address, .. } => Some(*address),
            TrayIconStyle::BatteryNumber { address, .. } => Some(*address),
            TrayIconStyle::BatteryRing { address, .. } => Some(*address),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Direction {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum ColorScheme {
    ConnectColor, // 连接状态颜色
    Custom,
    #[default]
    FollowSystemTheme, // 跟随系统主题
}

impl ColorScheme {
    pub fn is_connect_color(&self) -> bool {
        matches!(self, ColorScheme::ConnectColor)
    }

    pub fn is_custom(&self) -> bool {
        matches!(self, ColorScheme::Custom)
    }

    pub fn set_custom(&mut self) {
        *self = Self::Custom;
    }

    pub fn set_follow_system_theme(&mut self) {
        *self = Self::FollowSystemTheme;
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "style")]
pub enum TrayIconStyle {
    #[default]
    App,
    BatteryCustom {
        #[serde(rename = "bluetooth_address")]
        address: u64,
    },
    BatteryIcon {
        color_scheme: ColorScheme,
        #[serde(rename = "bluetooth_address")]
        address: u64,
        direction: Direction,
    },
    BatteryNumber {
        color_scheme: ColorScheme,
        #[serde(rename = "bluetooth_address")]
        address: u64,
        font_name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        font_color: Option</* Hex color */ String>,
    },
    BatteryRing {
        color_scheme: ColorScheme,
        #[serde(rename = "bluetooth_address")]
        address: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        highlight_color: Option</* Hex color */ String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        background_color: Option</* Hex color */ String>,
    },
}

impl TrayIconStyle {
    pub fn default_number_icon(address: u64, color_scheme: Option<ColorScheme>) -> Self {
        TrayIconStyle::BatteryNumber {
            address,
            color_scheme: color_scheme.unwrap_or_default(),
            font_name: "Arial".to_owned(),
            font_color: Some(String::new()),
        }
    }

    pub fn default_ring_icon(address: u64, color_scheme: Option<ColorScheme>) -> Self {
        TrayIconStyle::BatteryRing {
            address,
            color_scheme: color_scheme.unwrap_or_default(),
            highlight_color: Some(String::new()),
            background_color: Some(String::new()),
        }
    }

    pub fn default_hor_battery_icon(address: u64, color_scheme: Option<ColorScheme>) -> Self {
        TrayIconStyle::BatteryIcon {
            address,
            color_scheme: color_scheme.unwrap_or_default(),
            direction: Direction::Horizontal,
        }
    }

    pub fn default_vrt_battery_icon(address: u64, color_scheme: Option<ColorScheme>) -> Self {
        TrayIconStyle::BatteryIcon {
            address,
            color_scheme: color_scheme.unwrap_or_default(),
            direction: Direction::Vertical,
        }
    }

    pub fn update_address(&mut self, new_address: u64) -> bool {
        match self {
            Self::App => false,
            Self::BatteryCustom { address }
            | Self::BatteryIcon { address, .. }
            | Self::BatteryNumber { address, .. }
            | Self::BatteryRing { address, .. } => {
                *address = new_address;
                true
            }
        }
    }

    pub fn get_address(&self) -> Option<u64> {
        match self {
            Self::App => None,
            Self::BatteryCustom { address }
            | Self::BatteryIcon { address, .. }
            | Self::BatteryNumber { address, .. }
            | Self::BatteryRing { address, .. } => Some(*address),
        }
    }

    pub fn get_color_scheme(&self) -> Option<ColorScheme> {
        match self {
            Self::App | Self::BatteryCustom { .. } => None,
            Self::BatteryIcon { color_scheme, .. }
            | Self::BatteryNumber { color_scheme, .. }
            | Self::BatteryRing { color_scheme, .. } => Some(color_scheme.clone()),
        }
    }

    pub fn set_connect_color(&mut self, should_set: bool) {
        match self {
            Self::BatteryNumber { color_scheme, .. }
            | Self::BatteryIcon { color_scheme, .. }
            | Self::BatteryRing { color_scheme, .. } => {
                if should_set {
                    *color_scheme = ColorScheme::ConnectColor;
                } else {
                    *color_scheme = ColorScheme::FollowSystemTheme;
                }
            }
            _ => (),
        }
    }
}

#[derive(Debug, Clone, Copy, Setters, Getters, CopyGetters, Serialize, Deserialize)]
#[getset(set = "pub", get_copy = "pub")]
pub struct LowBattery {
    pub notify: bool,
    pub value: u8,
}

impl Default for LowBattery {
    fn default() -> Self {
        Self {
            notify: true,
            value: 15,
        }
    }
}

#[derive(Debug, Default, Setters, Getters, CopyGetters, Serialize, Deserialize)]
#[getset(set = "pub", get_copy = "pub")]
pub struct NotifyOptions {
    pub low_battery: LowBattery,
    pub disconnection: bool,
    pub reconnection: bool,
    pub added: bool,
    pub removed: bool,
    pub stay_on_screen: bool,
}

#[derive(Debug, Default, Setters, Getters, CopyGetters, Serialize, Deserialize)]
#[getset(set = "pub", get_copy = "pub")]
pub struct TooltipOptions {
    pub prefix_battery: bool,
    pub show_disconnected: bool,
    pub truncate_name: bool,
}

#[derive(Default, Getters, CopyGetters, Setters, Debug, Serialize, Deserialize)]
pub struct TrayOptions {
    #[serde(rename = "tooltip")]
    pub tooltip_options: TooltipOptions,
    #[serde(rename = "icon")]
    pub tray_icon_style: TrayIconStyle,
    #[getset(set = "pub", get_copy = "pub")]
    pub show_lowest_battery_device: bool,
}

fn find_custom_icon() -> Result<()> {
    let assets_path = ASSETS_PATH.clone();

    if !assets_path.is_dir() {
        return Err(anyhow!("Assets directory does not exist: {assets_path:?}"));
    }

    let have_custom_default_icons = (0..=100).all(|i| {
        let file_name = format!("{i}.png");
        let file_path = assets_path.join(file_name);
        file_path.is_file()
    });

    if have_custom_default_icons {
        return Ok(());
    }

    let have_custom_theme_icons = (0..=100).all(|i| {
        let file_dark_name = format!("{i}_dark.png");
        let file_light_name = format!("{i}_light.png");
        let file_dark_path = assets_path.join(file_dark_name);
        let file_light_path = assets_path.join(file_light_name);
        file_dark_path.is_file() || file_light_path.is_file()
    });

    if have_custom_theme_icons {
        return Ok(());
    }

    Err(anyhow!(
        "Assets directory does not contain custom battery icons."
    ))
}
