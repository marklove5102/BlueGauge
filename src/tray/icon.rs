use crate::{
    config::{ASSETS_PATH, CONFIG, Direction, TrayIconStyle},
    theme::SystemTheme,
};

use std::{
    path::{Path, PathBuf},
    sync::LazyLock,
};

use ab_glyph::{Font, FontVec, Glyph, GlyphId, PxScale, point};
use anyhow::{Context, Result, anyhow};
use image::Rgba;
use piet_common::{Color, Device, ImageFormat, LineCap, RenderContext, StrokeStyle};
use tray_icon::Icon;

static FONT_ARIAL_PATH: &str = r"C:\WINDOWS\FONTS\ARIAL.TTF";
static FONT_SEGOE_FLUENT_PATH: &str = r"C:\WINDOWS\FONTS\SEGOEICONS.TTF";
static FONT_SEGOE_MDL2_PATH: &str = r"C:\WINDOWS\FONTS\SEGMDL2.TTF";
static BATTERY_ICON_FONT_PATH: LazyLock<String> = LazyLock::new(|| {
    // Win11 使用 [Segoe Fluent Icons] 字体
    // Win10 使用 [Segoe MDL2 Assets] 字体
    // 若 win10 用户想要使用 Fluent 电池图标，需自行下载字体
    if !Path::new(FONT_SEGOE_FLUENT_PATH).is_file() {
        check_font_exists("Segoe Fluent Icons")
            .or_else(|| check_font_exists("SegoeFluentIcons"))
            .or_else(|| check_font_exists("SEGOEICONS"))
            .unwrap_or(FONT_SEGOE_MDL2_PATH.to_owned())
    } else {
        FONT_SEGOE_FLUENT_PATH.to_owned()
    }
});

const LOGO_DATA: &[u8] = include_bytes!("../../assets/logo.ico");

pub fn load_icon(icon_date: &[u8]) -> Result<Icon> {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::load_from_memory(icon_date)
            .map_err(|e| anyhow!("Failed to load icon - {e}"))?
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    Icon::from_rgba(icon_rgba, icon_width, icon_height).with_context(|| "Failed to crate the logo")
}

pub fn load_app_icon() -> Result<Icon> {
    load_icon(LOGO_DATA).map_err(|e| anyhow!("Failed to load app icon - {e}"))
}

pub fn load_tray_icon(battery_level: u8, bluetooth_status: bool) -> Result<Icon> {
    let tray_icon_style = CONFIG.read().unwrap().tray_options.tray_icon_style.clone();
    let is_low_battery = battery_level <= CONFIG.read().unwrap().notify_options.low_battery.value();

    match tray_icon_style {
        TrayIconStyle::App => load_app_icon(),
        TrayIconStyle::BatteryCustom { .. } => load_custom_icon(battery_level),
        TrayIconStyle::BatteryIcon {
            address: _,
            color_scheme,
            direction,
        } => {
            let is_connect_color = color_scheme.is_connect_color().then_some(bluetooth_status);

            load_battery_icon(battery_level, is_low_battery, direction, is_connect_color)
        }
        TrayIconStyle::BatteryNumber {
            address: _,
            color_scheme,
            font_name,
            font_color,
        } => {
            let is_connect_color = color_scheme.is_connect_color().then_some(bluetooth_status);

            load_number_icon(battery_level, &font_name, font_color, is_connect_color)
        }
        TrayIconStyle::BatteryRing {
            address: _,
            color_scheme,
            highlight_color,
            background_color,
        } => {
            let is_connect_color = color_scheme.is_connect_color().then_some(bluetooth_status);

            load_ring_icon(
                battery_level,
                is_low_battery,
                highlight_color,
                background_color,
                is_connect_color,
            )
        }
    }
}

fn load_custom_icon(battery_level: u8) -> Result<Icon> {
    let custom_battery_icon_path = || {
        let icon_dir = &ASSETS_PATH;
        let default_icon_path = icon_dir.join(format!("{battery_level}.png"));
        if default_icon_path.is_file() {
            return Ok(default_icon_path);
        }
        let theme_icon_path = match SystemTheme::get() {
            SystemTheme::Light => icon_dir.join(format!("light\\{battery_level}.png")),
            SystemTheme::Dark => icon_dir.join(format!("dark\\{battery_level}.png")),
        };
        if theme_icon_path.is_file() {
            return Ok(theme_icon_path);
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Failed to find {battery_level} default/theme PNG in Bluegauge directory"),
        ))
    };

    let icon_data = std::fs::read(custom_battery_icon_path()?)?;

    load_icon(&icon_data)
}

fn load_battery_icon(
    battery_level: u8,
    is_low_battery: bool,
    direction: Direction,
    is_connect_color: Option<bool>,
) -> Result<Icon> {
    let (icon_rgba, icon_width, icon_height) =
        render_battery_icon(battery_level, is_low_battery, direction, is_connect_color)?;
    Icon::from_rgba(icon_rgba, icon_width, icon_height)
        .map_err(|e| anyhow!("Failed to get Battery Icon - {e}"))
}

fn load_number_icon(
    battery_level: u8,
    font_name: &str,
    font_color: Option<String>,
    is_connect_color: Option<bool>,
) -> Result<Icon> {
    let (icon_rgba, icon_width, icon_height) =
        render_number_icon(battery_level, font_name, font_color, is_connect_color)?;
    Icon::from_rgba(icon_rgba, icon_width, icon_height)
        .map_err(|e| anyhow!("Failed to get Number Icon - {e}"))
}

pub fn load_ring_icon(
    battery_level: u8,
    is_low_battery: bool,
    highlight_color: Option</* Hex color */ String>,
    background_color: Option</* Hex color */ String>,
    is_connect_color: Option<bool>,
) -> Result<Icon> {
    let (icon_rgba, icon_width, icon_height) = render_ring_icon(
        battery_level,
        is_low_battery,
        highlight_color,
        background_color,
        is_connect_color,
    )?;
    Icon::from_rgba(icon_rgba, icon_width, icon_height)
        .map_err(|e| anyhow!("Failed to get Icon - {e}"))
}

fn render_battery_icon(
    battery_level: u8,
    is_low_battery: bool,
    direction: Direction,
    is_connect_color: Option<bool>,
) -> Result<(Vec<u8>, u32, u32)> {
    let font_path = BATTERY_ICON_FONT_PATH.as_str();
    let font_data = std::fs::read(font_path)?;
    let font = FontVec::try_from_vec(font_data).context("Failed to parse font")?;

    let font_color = {
        let base_color = if is_low_battery {
            Rgba([254, 102, 102, 255])
        } else {
            SystemTheme::get().get_font_color()
        };

        match is_connect_color {
            Some(true) => base_color,
            Some(false) => Rgba([base_color[0], base_color[1], base_color[2], 128]),
            None => base_color,
        }
    };

    let indicator = if battery_level == 0 {
        if direction == Direction::Horizontal {
            String::from('\u{eba0}')
        } else {
            String::from('\u{f5f2}')
        }
    } else {
        let ICONS: [char; 11] = if direction == Direction::Horizontal {
            [
                '\u{eba1}', // 1-10
                '\u{eba2}', // 11-20
                '\u{eba3}', // 21-30
                '\u{eba4}', // 31-40
                '\u{eba5}', // 41-50
                '\u{eba6}', // 51-60
                '\u{eba7}', // 61-70
                '\u{eba8}', // 71-80
                '\u{eba9}', // 81-90
                '\u{ebaa}', // 91-100
                '\u{ec02}', // Unknown
            ]
        } else {
            [
                '\u{f5f3}', // 1-10
                '\u{f5f4}', // 11-20
                '\u{f5f5}', // 21-30
                '\u{f5f6}', // 31-40
                '\u{f5f7}', // 41-50
                '\u{f5f8}', // 51-60
                '\u{f5f9}', // 61-70
                '\u{f5fa}', // 71-80
                '\u{f5fb}', // 81-90
                '\u{f5fc}', // 91-100
                '\u{f608}', // Unknown
            ]
        };
        ICONS[((battery_level - 1) / 10).min(10) as usize].to_string()
    };

    render_font(font, font_color, &indicator).map_err(|e| anyhow!("{e}"))
}

fn render_number_icon(
    battery_level: u8,
    font_name: &str,
    font_color: Option</* Hex color */ String>,
    is_connect_color: Option<bool>,
) -> Result<(Vec<u8>, u32, u32)> {
    let font_path = if font_name.trim().is_empty() {
        FONT_ARIAL_PATH.to_owned()
    } else {
        check_font_exists(font_name).unwrap_or(FONT_ARIAL_PATH.to_owned())
    };
    let font_data = std::fs::read(font_path)?;
    let font = FontVec::try_from_vec(font_data).context("Failed to parse font")?;

    let font_color = if let Some(should) = is_connect_color {
        if should {
            Rgba([79, 196, 120, 255])
        } else {
            Rgba([254, 102, 102, 255])
        }
    } else {
        font_color
            .and_then(|c| Color::from_hex_str(&c).ok())
            .map(|font_color| {
                let color = font_color.as_rgba8();
                Rgba([color.0, color.1, color.2, color.3])
            })
            .unwrap_or_else(|| SystemTheme::get().get_font_color())
    };

    let indicator = battery_level.to_string();

    render_font(font, font_color, &indicator).map_err(|e| anyhow!("{e}"))
}

fn render_ring_icon(
    battery_level: u8,
    is_low_battery: bool,
    highlight_color: Option</* Hex color */ String>,
    background_color: Option</* Hex color */ String>,
    is_connect_color: Option<bool>,
) -> Result<(Vec<u8>, u32, u32)> {
    let width = 64;
    let height = 64;

    let mut device = Device::new().map_err(|e| anyhow!("Failed to get Device - {e}"))?;
    let mut bitmap_target = device
        .bitmap_target(width, height, 1.0)
        .map_err(|e| anyhow!("Failed to create a new bitmap target. - {e}"))?;
    let mut piet = bitmap_target.render_context();

    let center = (32.0, 32.0);
    let inner_radius = 20.0;
    let outer_radius = 30.0;
    let stroke_width = outer_radius - inner_radius;

    // 使用平均半径作为圆弧半径
    let arc_radius = (inner_radius + outer_radius) / 2.0;

    // 将电量转换为百分比并计算对应的角度
    let battery_angle = battery_level as f64 * 3.6;
    let battery_angle_rad = battery_angle.to_radians();

    // 定义圆环的样式（圆角端点）
    let style = StrokeStyle::new().line_cap(LineCap::Round);

    // 起始角度（顶部，-90°）
    let start_angle_rad = -std::f64::consts::PI / 2.0;

    // 间隙角度转换为弧度
    let gap_angle: f64 = if battery_level > 90 { 0.0 } else { 30.0 };
    let gap_angle_rad = gap_angle.to_radians();

    // 计算每个圆环应该缩短的角度（各分摊一半的间隙）
    let shorten_angle_rad = gap_angle_rad / 2.0;
    let not_custome_color = || {
        let is_connect = is_connect_color.unwrap_or(true); // None 视为 默认连接
        if is_connect {
            match SystemTheme::get() {
                SystemTheme::Light => Color::from_rgba32_u32(0x919191FF),
                SystemTheme::Dark => Color::from_rgba32_u32(0xDADADAFF),
            }
        } else {
            match SystemTheme::get() {
                SystemTheme::Light => Color::from_rgba32_u32(0xC4C4C4FF),
                SystemTheme::Dark => Color::from_rgba32_u32(0xDADADAA0),
            }
        }
    };
    // 绘制背景圆环（表示剩余电量）
    let background_sweep_angle =
        2.0 * std::f64::consts::PI - battery_angle_rad - 2.0 * shorten_angle_rad;
    let background_color = background_color
        .and_then(|hex| Color::from_hex_str(&hex).ok()) // 优先配置颜色
        .unwrap_or_else(not_custome_color);
    let background_arc = piet_common::kurbo::Arc {
        center: center.into(),
        radii: piet_common::kurbo::Vec2::new(arc_radius, arc_radius),
        start_angle: start_angle_rad + battery_angle_rad + shorten_angle_rad,
        sweep_angle: background_sweep_angle,
        x_rotation: 0.0,
    };
    piet.stroke_styled(background_arc, &background_color, stroke_width, &style);

    // 绘制高亮圆环（表示当前电量）
    let highlight_color = if is_low_battery {
        // 低电量颜色（不支持配置中自定义）
        is_connect_color
            .and_then(|is_connect| {
                is_connect
                    .then_some(Color::from_rgba32_u32(0xFE6666FF))
                    .or(Some(Color::from_rgba32_u32(0xFE6666C0)))
            })
            .unwrap_or(Color::from_rgba32_u32(0xFE6666FF))
    } else {
        highlight_color
            .and_then(|hex| Color::from_hex_str(&hex).ok()) // 优先配置颜色
            .unwrap_or_else(|| {
                is_connect_color
                    .and_then(|is_connect| {
                        is_connect
                            .then_some(Color::from_rgba32_u32(0x4CD083FF))
                            .or(Some(Color::from_rgba32_u32(0x4CD083A0)))
                    })
                    .unwrap_or(Color::from_rgba32_u32(0x4CD083FF))
            })
    };
    let highlight_arc = piet_common::kurbo::Arc {
        center: center.into(),
        radii: piet_common::kurbo::Vec2::new(arc_radius, arc_radius),
        start_angle: start_angle_rad + shorten_angle_rad,
        sweep_angle: battery_angle_rad - 2.0 * shorten_angle_rad,
        x_rotation: 0.0,
    };
    piet.stroke_styled(highlight_arc, &highlight_color, stroke_width, &style);

    piet.finish().map_err(|e| anyhow!("{e}"))?;
    drop(piet);

    let image_buf = bitmap_target.to_image_buf(ImageFormat::RgbaPremul).unwrap();

    Ok((
        image_buf.raw_pixels().to_vec(),
        image_buf.width() as u32,
        image_buf.height() as u32,
    ))
}

pub fn render_font(
    font: FontVec,
    color: Rgba<u8>,
    text: &str,
) -> Result<(Vec<u8>, u32, u32), Box<dyn std::error::Error>> {
    let font_px = 36.0_f32;

    // --- compute conversion factor from font's "unscaled units" -> px ---
    // units_per_em is typically 1000 or 2048 depending on font.
    let units_per_em = font.units_per_em().unwrap_or(1000.0_f32);
    let scale_factor = font_px / units_per_em; // unscaled_value * scale_factor -> pixels

    // PxScale passed to Glyph (outline renderer) should be in pixels
    let px_scale = PxScale::from(font_px);

    // ---------- layout (simple horizontal) ----------
    let mut glyphs: Vec<Glyph> = Vec::new();
    let mut pen_x: f32 = 0.0;
    let mut prev_gid: Option<GlyphId> = None;

    for ch in text.chars() {
        let gid = font.glyph_id(ch);

        // apply kerning (unscaled kern * scale_factor -> px)
        if let Some(prev) = prev_gid {
            pen_x += font.kern_unscaled(prev, gid) * scale_factor;
        }
        prev_gid = Some(gid);

        // create glyph positioned at pen_x, baseline at ascent (converted to px)
        let glyph = Glyph {
            id: gid,
            scale: px_scale,
            position: point(pen_x, font.ascent_unscaled() * scale_factor),
        };

        // advance pen by advance (unscaled * scale_factor -> px)
        pen_x += font.h_advance_unscaled(gid) * scale_factor;

        glyphs.push(glyph);
    }

    // ---------- collect outlines & bounding box ----------
    let mut outlined = Vec::new();
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for g in &glyphs {
        if let Some(out) = font.outline_glyph(g.clone()) {
            let bb = out.px_bounds();
            min_x = min_x.min(bb.min.x);
            min_y = min_y.min(bb.min.y);
            max_x = max_x.max(bb.max.x);
            max_y = max_y.max(bb.max.y);
            outlined.push(out);
        }
    }

    if !min_x.is_finite() {
        return Ok((vec![0, 0, 0, 0], 1, 1));
    }

    // ---------- tight size ----------
    let width = max_x - min_x;
    let height = max_y - min_y;
    let side = width.max(height).ceil().max(1.0) as u32;

    // center offset to make it square
    let dx = ((side as f32 - width) / 2.0) - min_x;
    let dy = ((side as f32 - height) / 2.0) - min_y;

    // RGBA32 buffer
    let mut rgba = vec![0u8; (side * side * 4) as usize];

    // ---------- draw ----------
    for og in outlined {
        let bb = og.px_bounds();
        let x0 = (bb.min.x + dx).floor() as i32;
        let y0 = (bb.min.y + dy).floor() as i32;

        og.draw(|gx, gy, v| {
            let px = x0 + gx as i32;
            let py = y0 + gy as i32;
            if px < 0 || py < 0 {
                return;
            }
            let px = px as u32;
            let py = py as u32;
            if px >= side || py >= side {
                return;
            }

            let offset = ((py * side + px) * 4) as usize;

            let src_a = v * (color[3] as f32 / 255.0);
            if src_a <= 0.0 {
                return;
            }

            let dst_r = rgba[offset] as f32;
            let dst_g = rgba[offset + 1] as f32;
            let dst_b = rgba[offset + 2] as f32;
            let dst_a = rgba[offset + 3] as f32 / 255.0;

            let out_a = src_a + dst_a * (1.0 - src_a);

            let blend = |src: u8, dst: f32| -> u8 {
                ((src as f32 * src_a + dst * dst_a * (1.0 - src_a)) / out_a).clamp(0.0, 255.0) as u8
            };

            rgba[offset] = blend(color[0], dst_r);
            rgba[offset + 1] = blend(color[1], dst_g);
            rgba[offset + 2] = blend(color[2], dst_b);
            rgba[offset + 3] = (out_a * 255.0).clamp(0.0, 255.0) as u8;
        });
    }

    Ok((rgba, side, side))
}

fn check_font_exists(name: &str) -> Option<String> {
    let file_name =
        if Path::new(name).is_file() && (name.ends_with(".ttf") || name.ends_with(".otf")) {
            return Some(name.to_string());
        } else if name.ends_with(".ttf") || name.ends_with(".otf") {
            name.to_string()
        } else {
            format!("{}.tff", name.trim())
        };
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());
    let fonts_dir = Path::new(&windir).join("Fonts");
    let font_path = fonts_dir.join(&file_name);

    Path::new(&font_path)
        .is_file()
        .then_some(font_path)
        .or_else(|| {
            std::env::var_os("LOCALAPPDATA").and_then(|local_appdata| {
                let font_path = PathBuf::from(local_appdata)
                    .join("Microsoft")
                    .join("Windows")
                    .join("Fonts")
                    .join(file_name);
                font_path.is_file().then_some(font_path)
            })
        })
        .map(|p| p.to_string_lossy().to_string())
}
