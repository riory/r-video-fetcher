use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DEFAULT_MAX_RETRIES: u8 = 3;
pub const MAX_RETRIES: u8 = 10;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Settings {
    pub download_root: PathBuf,
    pub max_retries: u8,
    pub video_resolution: VideoResolution,
    pub audio_sample_rate: AudioSampleRate,
    pub audio_channels: AudioChannels,
    pub language: Language,
    pub recent_files: Vec<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            download_root: PathBuf::new(),
            max_retries: DEFAULT_MAX_RETRIES,
            video_resolution: VideoResolution::Best,
            audio_sample_rate: AudioSampleRate::Hz48000,
            audio_channels: AudioChannels::Stereo,
            language: Language::English,
            recent_files: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MediaOverrides {
    pub video_resolution: VideoResolution,
    pub audio_sample_rate: AudioSampleRate,
    pub audio_channels: AudioChannels,
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
    Any,
    Hz44100,
    #[default]
    Hz48000,
}

impl AudioSampleRate {
    pub fn hz(self) -> Option<u32> {
        match self {
            Self::Any => None,
            Self::Hz44100 => Some(44_100),
            Self::Hz48000 => Some(48_000),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioChannels {
    Any,
    Mono,
    #[default]
    Stereo,
}

impl AudioChannels {
    pub fn count(self) -> Option<u8> {
        match self {
            Self::Any => None,
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

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaType {
    SingleVideo,
    Playlist,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HistoryRecord {
    pub batch_name: String,
    pub url: String,
    pub created_at_unix_seconds: u64,
    pub size_bytes: u64,
    pub download_log: Vec<String>,
    #[serde(default)]
    pub target_folder: PathBuf,
    #[serde(default)]
    pub media_type: Option<MediaType>,
}
