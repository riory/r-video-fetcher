use crate::{
    model::{HistoryRecord, MediaType, Settings},
    storage,
};
use anyhow::{Context, Result, anyhow, bail};
use fs2::available_space;
use serde_json::Value;
use std::{
    fs,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::mpsc::Sender,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use url::Url;

const OUTPUT_MARKER: &str = "__R_VIDEO_FETCHER_OUTPUT__";
const INSPECTION_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Clone, Debug)]
pub struct MediaItem {
    pub title: String,
    pub url: String,
    pub channel: String,
    pub estimated_bytes: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct DownloadPlan {
    pub source_url: String,
    pub batch_name: String,
    pub playlist_name: Option<String>,
    pub items: Vec<MediaItem>,
    pub estimated_bytes: u64,
    pub unknown_size_count: usize,
    pub free_bytes: u64,
}

#[derive(Debug)]
pub enum DownloadEvent {
    PlanReady(Result<DownloadPlan, String>),
    AttemptStarted {
        index: usize,
        attempt: u8,
        total_attempts: u8,
    },
    Progress {
        index: usize,
        percent: f32,
    },
    ItemCompleted {
        index: usize,
        size_bytes: u64,
    },
    ItemFailed {
        index: usize,
        message: String,
    },
    Finished(Result<HistoryRecord, String>),
}

pub fn inspect_batch(source_url: &str, settings: &Settings) -> Result<DownloadPlan> {
    let parsed_url = Url::parse(source_url).context("Enter a valid HTTP or HTTPS URL.")?;
    if !matches!(parsed_url.scheme(), "http" | "https") || parsed_url.host_str().is_none() {
        bail!("Enter a valid HTTP or HTTPS URL.");
    }
    if settings.download_root.as_os_str().is_empty() {
        bail!("Choose a download folder before previewing.");
    }
    if settings.download_root.exists() && !settings.download_root.is_dir() {
        bail!(
            "The download location is not a folder: {}.",
            settings.download_root.display()
        );
    }

    check_tool("yt-dlp", &["--version"])?;
    check_tool("ffmpeg", &["-version"])?;

    let mut command = Command::new("yt-dlp");
    command
        .args([
            "--no-warnings",
            "--no-call-home",
            "--skip-download",
            "--dump-single-json",
            "--socket-timeout",
            "10",
            "--retries",
            "1",
            "--extractor-retries",
            "1",
        ])
        .arg(source_url);
    let output = output_with_timeout(&mut command, INSPECTION_TIMEOUT)
        .context("Could not inspect the URL with yt-dlp.")?;
    if !output.status.success() {
        bail!(
            "yt-dlp could not inspect the URL: {}",
            diagnostic_text(&output.stderr)
        );
    }
    let metadata: Value = serde_json::from_slice(&output.stdout)
        .context("yt-dlp returned invalid metadata for this URL.")?;
    let (batch_name, playlist_name, items) = parse_metadata(&metadata, source_url)?;
    let estimated_bytes = items
        .iter()
        .filter_map(|item| item.estimated_bytes)
        .fold(0u64, u64::saturating_add);
    let unknown_size_count = items
        .iter()
        .filter(|item| item.estimated_bytes.is_none())
        .count();
    let free_bytes = disk_space_available(&settings.download_root)?;

    Ok(DownloadPlan {
        source_url: source_url.to_owned(),
        batch_name,
        playlist_name,
        items,
        estimated_bytes,
        unknown_size_count,
        free_bytes,
    })
}

pub fn parse_metadata(
    metadata: &Value,
    source_url: &str,
) -> Result<(String, Option<String>, Vec<MediaItem>)> {
    let (raw_items, is_playlist) = metadata
        .get("entries")
        .and_then(Value::as_array)
        .map(|entries| (entries.iter().collect::<Vec<_>>(), true))
        .unwrap_or_else(|| (vec![metadata], false));
    let playlist_name = is_playlist
        .then(|| string_field(metadata, &["title"]).unwrap_or_else(|| "Playlist".to_owned()));
    let default_channel =
        string_field(metadata, &["uploader_id", "channel", "uploader", "creator"])
            .map(strip_channel_prefix);

    let mut items = Vec::with_capacity(raw_items.len());
    for (index, item) in raw_items.into_iter().enumerate() {
        if item.is_null() {
            continue;
        }
        let video_url = item
            .get("webpage_url")
            .or_else(|| item.get("original_url"))
            .or_else(|| item.get("url"))
            .and_then(Value::as_str)
            .and_then(|url| resolve_url(source_url, url))
            .ok_or_else(|| {
                anyhow!(
                    "Could not find a video URL for playlist item {}.",
                    index + 1
                )
            })?;
        let title =
            string_field(item, &["title", "id"]).unwrap_or_else(|| format!("Video {}", index + 1));
        let channel = string_field(item, &["uploader_id", "channel", "uploader", "creator"])
            .or_else(|| default_channel.clone())
            .map(strip_channel_prefix)
            .unwrap_or_else(|| "Unknown channel".to_owned());
        items.push(MediaItem {
            title,
            url: video_url,
            channel,
            estimated_bytes: estimate_item_size(item),
        });
    }

    if items.is_empty() {
        bail!("No downloadable videos were found at this URL.");
    }
    let batch_name = playlist_name
        .clone()
        .or_else(|| items.first().map(|item| item.title.clone()))
        .unwrap_or_else(|| "Download batch".to_owned());
    Ok((batch_name, playlist_name, items))
}

pub fn download_batch(
    plan: DownloadPlan,
    settings: Settings,
    events: Sender<DownloadEvent>,
) -> Result<HistoryRecord> {
    let total_attempts = settings.max_retries.saturating_add(1).max(1);
    let mut log = vec![format!(
        "Starting batch '{}' with {} video(s).",
        plan.batch_name,
        plan.items.len()
    )];
    let mut total_size = 0u64;

    for (index, item) in plan.items.iter().enumerate() {
        let mut final_result = None;
        for attempt in 1..=total_attempts {
            let _ = events.send(DownloadEvent::AttemptStarted {
                index,
                attempt,
                total_attempts,
            });
            match download_item(index, item, &plan, &settings, &events) {
                Ok((path, size_bytes)) => {
                    total_size = total_size.saturating_add(size_bytes);
                    log.push(format!(
                        "Downloaded '{}' ({} bytes) to {}.",
                        item.title,
                        size_bytes,
                        path.display()
                    ));
                    let _ = events.send(DownloadEvent::ItemCompleted { index, size_bytes });
                    final_result = Some(Ok(()));
                    break;
                }
                Err(error) => {
                    let message = format!("{error:#}");
                    log.push(format!(
                        "Attempt {attempt}/{total_attempts} failed for '{}': {message}",
                        item.title
                    ));
                    if attempt < total_attempts {
                        let _ = events.send(DownloadEvent::ItemFailed {
                            index,
                            message: format!(
                                "Attempt {attempt}/{total_attempts} failed; retrying: {message}"
                            ),
                        });
                    }
                    final_result = Some(Err(message));
                }
            }
        }
        if let Some(Err(message)) = final_result {
            log.push(format!(
                "Failed '{}' after {total_attempts} attempt(s): {message}",
                item.title
            ));
            let _ = events.send(DownloadEvent::ItemFailed {
                index,
                message: format!("Failed after {total_attempts} attempt(s): {message}"),
            });
        }
    }

    let created_at_unix_seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("System clock is earlier than the Unix epoch.")?
        .as_secs();
    let record = HistoryRecord {
        batch_name: plan.batch_name,
        url: plan.source_url,
        created_at_unix_seconds,
        size_bytes: total_size,
        download_log: log,
        target_folder: settings.download_root,
        media_type: Some(if plan.playlist_name.is_some() {
            MediaType::Playlist
        } else {
            MediaType::SingleVideo
        }),
    };
    storage::save_history(&record)?;
    Ok(record)
}

pub fn format_selector(settings: &Settings) -> String {
    let height_filter = settings
        .video_resolution
        .height_limit()
        .map(|height| format!("[height<={height}]"))
        .unwrap_or_default();
    let mut audio_filters = String::new();
    if let Some(sample_rate) = settings.audio_sample_rate.hz() {
        audio_filters.push_str(&format!("[asr={sample_rate}]"));
    }
    if let Some(channels) = settings.audio_channels.count() {
        audio_filters.push_str(&format!("[audio_channels={channels}]"));
    }

    if height_filter.is_empty() {
        format!("bestvideo+bestaudio{audio_filters}/best")
    } else {
        format!("bestvideo{height_filter}+bestaudio{audio_filters}/best{height_filter}/best")
    }
}

pub fn parse_progress_percent(line: &str) -> Option<f32> {
    let download_line = line.split_once("[download]")?.1;
    let percent_text = download_line.split_once('%')?.0.trim();
    let percent = percent_text.parse::<f32>().ok()?;
    (percent.is_finite() && (0.0..=100.0).contains(&percent)).then_some(percent)
}

pub fn sanitize_path_component(value: &str) -> String {
    let mut sanitized = String::with_capacity(value.len());
    for character in value.chars() {
        if matches!(character, '\'' | '’') {
            continue;
        }
        let component = if matches!(
            character,
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' | '%'
        ) || character.is_control()
            || character.is_whitespace()
            || (!character.is_alphanumeric() && !matches!(character, '.' | '-' | '_'))
        {
            '_'
        } else {
            character
        };
        if component != '_' || !sanitized.ends_with('_') {
            sanitized.push(component);
        }
    }
    let sanitized = sanitized
        .trim_matches(|character| matches!(character, '.' | ' '))
        .to_owned();
    if sanitized.is_empty() {
        "unknown".to_owned()
    } else {
        sanitized
    }
}

pub fn download_output_directory(
    root: &Path,
    item: &MediaItem,
    playlist_name: Option<&str>,
) -> PathBuf {
    let mut output_directory = root.join(sanitize_path_component(&item.channel));
    if let Some(playlist_name) = playlist_name {
        output_directory = output_directory.join(sanitize_path_component(playlist_name));
    }
    output_directory
}

fn download_item(
    index: usize,
    item: &MediaItem,
    plan: &DownloadPlan,
    settings: &Settings,
    events: &Sender<DownloadEvent>,
) -> Result<(PathBuf, u64)> {
    let output_directory =
        download_output_directory(&settings.download_root, item, plan.playlist_name.as_deref());
    fs::create_dir_all(&output_directory).with_context(|| {
        format!(
            "Could not create download folder {}",
            output_directory.display()
        )
    })?;
    let output_template = output_directory.join("%(title)s [%(id)s].%(ext)s");
    let mut child = Command::new("yt-dlp")
        .args([
            "--no-warnings",
            "--no-call-home",
            "--no-playlist",
            "--newline",
            "--progress",
            "--windows-filenames",
            "--no-overwrites",
            "--format",
        ])
        .arg(format_selector(settings))
        .arg("--output")
        .arg(output_template)
        .arg("--print")
        .arg(format!("after_move:{OUTPUT_MARKER}%(filepath)s"))
        .arg(&item.url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Could not run yt-dlp. Install it and add it to PATH.")?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("Could not read yt-dlp progress output."))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("Could not read yt-dlp diagnostic output."))?;
    let progress_events = events.clone();
    let stderr_reader = thread::spawn(move || {
        let mut diagnostics = Vec::new();
        let mut reader = BufReader::new(stderr);
        let mut line = Vec::new();
        loop {
            line.clear();
            let bytes_read = reader.read_until(b'\n', &mut line)?;
            if bytes_read == 0 {
                break;
            }
            if let Some(percent) = parse_progress_percent(&String::from_utf8_lossy(&line)) {
                let _ = progress_events.send(DownloadEvent::Progress { index, percent });
            }
            diagnostics.extend_from_slice(&line);
        }
        Ok::<_, std::io::Error>(diagnostics)
    });

    let mut output_path = None;
    let mut read_error = None;
    let mut line = Vec::new();
    let mut stdout_reader = BufReader::new(stdout);
    loop {
        line.clear();
        match stdout_reader.read_until(b'\n', &mut line) {
            Ok(0) => break,
            Ok(_) => {
                let line_text = String::from_utf8_lossy(&line);
                if let Some(path) = line_text.trim().strip_prefix(OUTPUT_MARKER) {
                    output_path = Some(PathBuf::from(path));
                } else if let Some(percent) = parse_progress_percent(&line_text) {
                    let _ = events.send(DownloadEvent::Progress { index, percent });
                }
            }
            Err(error) => {
                read_error = Some(error);
                break;
            }
        }
    }
    let status = child
        .wait()
        .context("Could not wait for yt-dlp to finish.")?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| anyhow!("Could not collect yt-dlp diagnostics."))?
        .context("Could not read yt-dlp diagnostics.")?;
    if let Some(error) = read_error {
        return Err(error).context("Could not read yt-dlp progress.");
    }
    if !status.success() {
        bail!("yt-dlp failed: {}", diagnostic_text(&stderr));
    }

    let output_path = output_path.ok_or_else(|| {
        anyhow!(
            "yt-dlp completed without reporting the output file for '{}'.",
            item.title
        )
    })?;
    let metadata = fs::metadata(&output_path).with_context(|| {
        format!(
            "Downloaded file was not found at {}.",
            output_path.display()
        )
    })?;
    if metadata.len() == 0 {
        bail!("Downloaded file is empty: {}", output_path.display());
    }
    verify_file(&output_path)?;
    Ok((output_path, metadata.len()))
}

fn verify_file(path: &Path) -> Result<()> {
    let output = Command::new("ffmpeg")
        .args(["-v", "error", "-xerror", "-i"])
        .arg(path)
        .args(["-f", "null", "-"])
        .output()
        .with_context(|| format!("Could not verify {} with ffmpeg.", path.display()))?;
    if !output.status.success() {
        bail!(
            "File verification failed for {}: {}",
            path.display(),
            diagnostic_text(&output.stderr)
        );
    }
    Ok(())
}

fn check_tool(program: &str, arguments: &[&str]) -> Result<()> {
    let output = Command::new(program)
        .args(arguments)
        .output()
        .with_context(|| format!("{program} was not found. Install it and add it to PATH."))?;
    if !output.status.success() {
        bail!(
            "{program} is installed but could not run: {}",
            diagnostic_text(&output.stderr)
        );
    }
    Ok(())
}

fn output_with_timeout(command: &mut Command, timeout: Duration) -> Result<Output> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Could not run yt-dlp. Install it and add it to PATH.")?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("Could not read yt-dlp output."))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("Could not read yt-dlp diagnostics."))?;
    let stdout_reader = thread::spawn(move || {
        let mut output = Vec::new();
        BufReader::new(stdout)
            .read_to_end(&mut output)
            .map(|_| output)
    });
    let stderr_reader = thread::spawn(move || {
        let mut output = Vec::new();
        BufReader::new(stderr)
            .read_to_end(&mut output)
            .map(|_| output)
    });
    let started = Instant::now();
    let status = loop {
        match child.try_wait().context("Could not check yt-dlp status.")? {
            Some(status) => break status,
            None if started.elapsed() >= timeout => {
                let kill_result = child.kill();
                let wait_result = child.wait();
                let stdout_result = join_command_reader(stdout_reader, "output");
                let stderr_result = join_command_reader(stderr_reader, "diagnostics");
                kill_result.context("Could not stop timed-out yt-dlp.")?;
                wait_result.context("Could not wait for timed-out yt-dlp.")?;
                stdout_result?;
                stderr_result?;
                bail!(
                    "yt-dlp inspection exceeded the {} second limit.",
                    timeout.as_secs()
                );
            }
            None => thread::sleep(Duration::from_millis(100)),
        }
    };
    Ok(Output {
        status,
        stdout: join_command_reader(stdout_reader, "output")?,
        stderr: join_command_reader(stderr_reader, "diagnostics")?,
    })
}

fn join_command_reader(
    reader: thread::JoinHandle<std::io::Result<Vec<u8>>>,
    output_name: &str,
) -> Result<Vec<u8>> {
    reader
        .join()
        .map_err(|_| anyhow!("Could not collect yt-dlp {output_name}."))?
        .with_context(|| format!("Could not read yt-dlp {output_name}."))
}

fn disk_space_available(path: &Path) -> Result<u64> {
    let mut candidate = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()
            .context("Could not determine current folder.")?
            .join(path)
    };
    loop {
        if candidate.exists() && candidate.is_dir() {
            return available_space(&candidate).with_context(|| {
                format!(
                    "Could not check free disk space at {}.",
                    candidate.display()
                )
            });
        }
        candidate = candidate
            .parent()
            .ok_or_else(|| {
                anyhow!(
                    "Could not find an existing folder to check free space for {}.",
                    path.display()
                )
            })?
            .to_owned();
    }
}

fn resolve_url(source_url: &str, candidate: &str) -> Option<String> {
    if let Ok(url) = Url::parse(candidate) {
        return matches!(url.scheme(), "http" | "https").then(|| url.to_string());
    }
    Url::parse(source_url)
        .ok()?
        .join(candidate)
        .ok()
        .filter(|url| matches!(url.scheme(), "http" | "https"))
        .map(|url| url.to_string())
}

fn string_field(value: &Value, fields: &[&str]) -> Option<String> {
    fields
        .iter()
        .find_map(|field| value.get(*field).and_then(Value::as_str))
        .map(str::to_owned)
}

fn strip_channel_prefix(channel: String) -> String {
    channel.strip_prefix('@').unwrap_or(&channel).to_owned()
}

fn estimate_item_size(item: &Value) -> Option<u64> {
    item.get("filesize")
        .or_else(|| item.get("filesize_approx"))
        .and_then(Value::as_u64)
        .or_else(|| {
            item.get("requested_formats")
                .and_then(Value::as_array)
                .map(|formats| {
                    formats
                        .iter()
                        .filter_map(|format| {
                            format
                                .get("filesize")
                                .or_else(|| format.get("filesize_approx"))
                                .and_then(Value::as_u64)
                        })
                        .fold(0u64, u64::saturating_add)
                })
                .filter(|total| *total > 0)
        })
}

fn diagnostic_text(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        "No additional details were provided.".to_owned()
    } else {
        let mut tail = trimmed.chars().rev().take(2000).collect::<String>();
        if trimmed.chars().count() > 2000 {
            tail.push_str("...");
        }
        tail.chars().rev().collect()
    }
}
