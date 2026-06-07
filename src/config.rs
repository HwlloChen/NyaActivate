use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub watermark: WatermarkConfig,
}

#[derive(Debug, Deserialize)]
pub struct WatermarkConfig {
    #[serde(default = "default_line1")]
    pub line1: String,
    #[serde(default = "default_line2")]
    pub line2: String,
    #[serde(default = "default_color")]
    pub color: String,
    #[serde(default = "default_font_size1")]
    pub font_size1: u32,
    #[serde(default = "default_font_size2")]
    pub font_size2: u32,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub colorful: bool,
    #[serde(default)]
    pub level: Level,
}

#[derive(Debug, Default, Deserialize, Clone, Copy, PartialEq)]
pub enum Level {
    #[default]
    #[serde(rename = "TopMost")]
    TopMost,
    #[serde(rename = "Desktop")]
    Desktop,
}

fn default_line1() -> String { "激活 Windows".to_string() }
fn default_line2() -> String { "转到“设置”以激活 Windows。".to_string() }
fn default_color() -> String { "#a6a7a8".to_string() }
fn default_font_size1() -> u32 { 18 }
fn default_font_size2() -> u32 { 13 }

pub fn parse_color(hex: &str) -> Option<u32> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 { return None; }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    // Win32 COLORREF: 0x00BBGGRR
    Some((r as u32) | ((g as u32) << 8) | ((b as u32) << 16))
}

pub fn hsl_to_rgb(h: f32, s: f32, l: f32) -> u32 {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h60 = h / 60.0;
    let x = c * (1.0 - (h60 % 2.0 - 1.0).abs());
    let m = l - c / 2.0;

    let (r, g, b) = match h60 as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    let r = ((r + m) * 255.0) as u8;
    let g = ((g + m) * 255.0) as u8;
    let b = ((b + m) * 255.0) as u8;

    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

impl Config {
    pub fn load<P: AsRef<Path>>(path: P) -> Self {
        let path = path.as_ref();
        if path.exists() {
            match std::fs::read_to_string(path) {
                Ok(content) => match toml::from_str(&content) {
                    Ok(config) => return config,
                    Err(e) => log::warn!("配置解析失败: {e}, 使用默认配置"),
                },
                Err(e) => log::warn!("无法读取配置文件: {e}, 使用默认配置"),
            }
        } else {
            log::info!("配置文件不存在于 {:?}, 使用默认配置", path);
        }
        Config::default()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self { watermark: WatermarkConfig::default() }
    }
}

impl Default for WatermarkConfig {
    fn default() -> Self {
        Self {
            line1: default_line1(),
            line2: default_line2(),
            color: default_color(),
            font_size1: default_font_size1(),
            font_size2: default_font_size2(),
            bold: false,
            colorful: false,
            level: Level::default(),
        }
    }
}
