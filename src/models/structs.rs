use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DEFAULT_MAX_RETRIES: u8 = 3;
pub const MAX_RETRIES: u8 = 10;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct Settings {
    pub download_root: PathBuf,
    pub max_retries: u8,
    pub file_type: FileType,
    pub theme: AppTheme,
    pub video_resolution: VideoResolution,
    pub audio_sample_rate: AudioSampleRate,
    pub audio_channels: AudioChannels,
    pub language: Language,
    pub recent_files: Vec<PathBuf>,
    pub window_state: Option<WindowState>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            download_root: PathBuf::new(),
            max_retries: DEFAULT_MAX_RETRIES,
            file_type: FileType::FromSourceVideo,
            theme: AppTheme::Default,
            video_resolution: VideoResolution::Best,
            audio_sample_rate: AudioSampleRate::FromSourceVideo,
            audio_channels: AudioChannels::FromSourceVideo,
            language: Language::English,
            recent_files: Vec::new(),
            window_state: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct WindowState {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl WindowState {
    pub fn clamp_to(self, screen_size: [f32; 2], decorations: [f32; 2]) -> Self {
        let screen_width = positive_finite(screen_size[0]);
        let screen_height = positive_finite(screen_size[1]);
        let decoration_width = nonnegative_finite(decorations[0]);
        let decoration_height = nonnegative_finite(decorations[1]);
        let max_width = (screen_width - decoration_width).max(1.0);
        let max_height = (screen_height - decoration_height).max(1.0);
        let width = finite_or(self.width, max_width).clamp(1.0, max_width);
        let height = finite_or(self.height, max_height).clamp(1.0, max_height);
        let max_x = (screen_width - decoration_width - width).max(0.0);
        let max_y = (screen_height - decoration_height - height).max(0.0);

        Self {
            x: finite_or(self.x, 0.0).clamp(0.0, max_x),
            y: finite_or(self.y, 0.0).clamp(0.0, max_y),
            width,
            height,
        }
    }
}

fn positive_finite(value: f32) -> f32 {
    finite_or(value, 1.0).max(1.0)
}

fn nonnegative_finite(value: f32) -> f32 {
    finite_or(value, 0.0).max(0.0)
}

fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct MediaOverrides {
    pub video_resolution: VideoResolution,
    pub audio_sample_rate: AudioSampleRate,
    pub audio_channels: AudioChannels,
}

impl From<&Settings> for MediaOverrides {
    fn from(settings: &Settings) -> Self {
        Self {
            video_resolution: settings.video_resolution,
            audio_sample_rate: settings.audio_sample_rate,
            audio_channels: settings.audio_channels,
        }
    }
}

impl Settings {
    pub fn apply_media_overrides(&mut self, overrides: MediaOverrides) {
        self.video_resolution = overrides.video_resolution;
        self.audio_sample_rate = overrides.audio_sample_rate;
        self.audio_channels = overrides.audio_channels;
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileType {
    #[default]
    FromSourceVideo,
    Mp4,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AppTheme {
    #[default]
    Default,
    Dark,
    Zenburn,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VideoResolution {
    #[default]
    Best,
    P360,
    P480,
    P720,
    P1080,
    P1440,
    P2160,
}

impl VideoResolution {
    pub fn height_limit(self) -> Option<u16> {
        match self {
            Self::Best => None,
            Self::P360 => Some(360),
            Self::P480 => Some(480),
            Self::P720 => Some(720),
            Self::P1080 => Some(1080),
            Self::P1440 => Some(1440),
            Self::P2160 => Some(2160),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioSampleRate {
    #[default]
    FromSourceVideo,
    Hz44100,
    Hz48000,
}

impl AudioSampleRate {
    pub fn hz(self) -> Option<u32> {
        match self {
            Self::FromSourceVideo => None,
            Self::Hz44100 => Some(44_100),
            Self::Hz48000 => Some(48_000),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioChannels {
    #[default]
    FromSourceVideo,
    Mono,
    Stereo,
}

impl AudioChannels {
    pub fn count(self) -> Option<u8> {
        match self {
            Self::FromSourceVideo => None,
            Self::Mono => Some(1),
            Self::Stereo => Some(2),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    #[default]
    English,
    Russian,
}
