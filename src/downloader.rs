use crate::{
    model::{FileType, Settings},
    models::project::{DownloadLogEntry, MediaType, Project},
    storage, tool_command,
};
use anyhow::{Context, Result, anyhow, bail};
use fs2::available_space;
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant},
};
use url::Url;

pub use crate::models::download::{DownloadPlan, MediaItem};

const OUTPUT_MARKER: &str = "__R_VIDEO_FETCHER_OUTPUT__";
const INSPECTION_TIMEOUT: Duration = Duration::from_secs(300);
const MAX_CONCURRENT_PLAYLIST_ITEMS: usize = 4;

#[derive(Debug)]
pub enum InspectionProgress {
    CheckingTools,
    FetchingMetadata,
    EstimatingItems {
        completed: usize,
        total: usize,
        active: usize,
        unavailable: usize,
    },
    CheckingDiskSpace,
}

#[derive(Debug)]
pub enum DownloadEvent {
    PlanReady(Result<DownloadPlan, String>),
    InspectionProgress(InspectionProgress),
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
    ItemVerifying {
        index: usize,
    },
    ItemAborted {
        index: usize,
    },
    ItemFailed {
        index: usize,
        message: String,
    },
    Finished(Result<Project, String>),
}

pub fn inspect_batch(source_url: &str, settings: &Settings) -> Result<DownloadPlan> {
    inspect_batch_with_progress(source_url, settings, |_| {})
}

pub fn inspect_batch_with_progress(
    source_url: &str,
    settings: &Settings,
    mut report_progress: impl FnMut(InspectionProgress),
) -> Result<DownloadPlan> {
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

    report_progress(InspectionProgress::CheckingTools);
    check_tool("yt-dlp", &["--version"])?;
    check_tool("ffmpeg", &["-version"])?;

    report_progress(InspectionProgress::FetchingMetadata);
    let mut command = tool_command::yt_dlp(&std::env::temp_dir());
    command
        .args([
            "--no-warnings",
            "--no-call-home",
            "--skip-download",
            "--flat-playlist",
            "--dump-single-json",
            "--socket-timeout",
            "10",
            "--retries",
            "1",
            "--extractor-retries",
            "1",
        ])
        .arg(source_url);
    let output = output_with_timeout(&mut command, INSPECTION_TIMEOUT, |_| {})
        .context("Could not inspect the URL with yt-dlp.")?;
    if !output.status.success() {
        bail!(
            "yt-dlp could not inspect the URL: {}",
            diagnostic_text(&output.stderr)
        );
    }
    let metadata: Value = serde_json::from_slice(&output.stdout)
        .context("yt-dlp returned invalid metadata for this URL.")?;
    let (batch_name, playlist_name, mut items) = parse_metadata(&metadata, source_url)?;
    estimate_missing_item_sizes(&mut items, &mut report_progress)?;
    let estimated_bytes = items
        .iter()
        .filter_map(|item| item.estimated_bytes)
        .fold(0u64, u64::saturating_add);
    let unknown_size_count = items
        .iter()
        .filter(|item| item.estimated_bytes.is_none())
        .count();
    report_progress(InspectionProgress::CheckingDiskSpace);
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

fn estimate_missing_item_sizes(
    items: &mut [MediaItem],
    report_progress: &mut impl FnMut(InspectionProgress),
) -> Result<()> {
    let pending_indices = items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| item.estimated_bytes.is_none().then_some(index))
        .collect::<Vec<_>>();
    if pending_indices.is_empty() {
        return Ok(());
    }

    let total = items.len();
    let mut completed = total - pending_indices.len();
    let mut active = 0;
    let mut unavailable = 0;
    let mut next_pending = 0;
    thread::scope(|scope| -> Result<()> {
        let (completed_tx, completed_rx) = mpsc::channel();

        while active < MAX_CONCURRENT_PLAYLIST_ITEMS && next_pending < pending_indices.len() {
            spawn_size_estimate(
                scope,
                pending_indices[next_pending],
                &items[pending_indices[next_pending]].url,
                completed_tx.clone(),
            );
            active += 1;
            next_pending += 1;
        }
        report_progress(InspectionProgress::EstimatingItems {
            completed,
            total,
            active,
            unavailable,
        });

        while active > 0 {
            let (index, result) = completed_rx
                .recv()
                .context("Size-estimation workers stopped before the playlist was estimated.")?;
            active -= 1;
            completed += 1;
            match result {
                Ok(Some(size_bytes)) => items[index].estimated_bytes = Some(size_bytes),
                Ok(None) | Err(_) => unavailable += 1,
            }

            while active < MAX_CONCURRENT_PLAYLIST_ITEMS && next_pending < pending_indices.len() {
                let index = pending_indices[next_pending];
                spawn_size_estimate(scope, index, &items[index].url, completed_tx.clone());
                active += 1;
                next_pending += 1;
            }
            report_progress(InspectionProgress::EstimatingItems {
                completed,
                total,
                active,
                unavailable,
            });
        }

        Ok(())
    })
}

fn spawn_size_estimate<'scope>(
    scope: &'scope thread::Scope<'scope, '_>,
    index: usize,
    url: &str,
    completed_tx: Sender<(usize, Result<Option<u64>>)>,
) {
    let url = url.to_owned();
    scope.spawn(move || {
        let result = estimate_item_size_from_url(&url);
        let _ = completed_tx.send((index, result));
    });
}

fn estimate_item_size_from_url(url: &str) -> Result<Option<u64>> {
    let mut command = tool_command::yt_dlp(&std::env::temp_dir());
    command
        .args([
            "--no-warnings",
            "--no-call-home",
            "--skip-download",
            "--no-playlist",
            "--dump-single-json",
            "--socket-timeout",
            "10",
            "--retries",
            "1",
            "--extractor-retries",
            "1",
        ])
        .arg(url);
    let output = output_with_timeout(&mut command, INSPECTION_TIMEOUT, |_| {})
        .with_context(|| format!("Could not estimate size for {url}."))?;
    if !output.status.success() {
        bail!(
            "yt-dlp could not estimate size for {url}: {}",
            diagnostic_text(&output.stderr)
        );
    }
    let metadata: Value = serde_json::from_slice(&output.stdout)
        .with_context(|| format!("yt-dlp returned invalid size metadata for {url}."))?;
    Ok(estimate_item_size(&metadata))
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
) -> Result<Project> {
    let (_abort_sender, abort_requests) = mpsc::channel();
    download_batch_with_abort(plan, settings, events, abort_requests)
}

pub fn download_batch_with_abort(
    plan: DownloadPlan,
    settings: Settings,
    events: Sender<DownloadEvent>,
    abort_requests: Receiver<usize>,
) -> Result<Project> {
    let total_attempts = settings.max_retries.saturating_add(1).max(1);
    let concurrency_limit = download_concurrency_limit(&plan);
    let item_results = thread::scope(|scope| -> Result<Vec<Option<ItemDownloadResult>>> {
        let (completed_tx, completed_rx) = mpsc::channel();
        let mut results = std::iter::repeat_with(|| None)
            .take(plan.items.len())
            .collect::<Vec<_>>();
        let mut active: HashMap<usize, Sender<()>> = HashMap::new();
        let mut pending_aborts = HashSet::new();
        let mut next_index = 0;

        loop {
            for index in abort_requests.try_iter() {
                if let Some(abort_tx) = active.get(&index) {
                    let _ = abort_tx.send(());
                } else if index >= next_index && index < plan.items.len() {
                    pending_aborts.insert(index);
                }
            }

            while active.len() < concurrency_limit && next_index < plan.items.len() {
                let index = next_index;
                next_index += 1;
                if pending_aborts.remove(&index) {
                    let item = &plan.items[index];
                    let _ = events.send(DownloadEvent::ItemAborted { index });
                    results[index] = Some(ItemDownloadResult {
                        outcome: ItemDownloadOutcome::Aborted,
                        log_entries: vec![DownloadLogEntry::GenericMessage {
                            message: format!("Aborted '{}'.", item.title),
                        }],
                    });
                    continue;
                }

                let (abort_tx, abort_rx) = mpsc::channel();
                active.insert(index, abort_tx);
                let item = &plan.items[index];
                let plan_ref = &plan;
                let settings_ref = &settings;
                let worker_events = events.clone();
                let worker_completed_tx = completed_tx.clone();
                scope.spawn(move || {
                    let result = download_item_with_retries(
                        index,
                        item,
                        plan_ref,
                        settings_ref,
                        &worker_events,
                        abort_rx,
                        total_attempts,
                    );
                    let _ = worker_completed_tx.send((index, result));
                });
            }

            if active.is_empty() && next_index == plan.items.len() {
                break;
            }

            match completed_rx.recv_timeout(Duration::from_millis(100)) {
                Ok((index, result)) => {
                    active.remove(&index);
                    results[index] = Some(result);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    bail!("Download workers stopped before the batch completed.")
                }
            }
        }

        Ok(results)
    })?;

    let mut log = vec![DownloadLogEntry::GenericMessage {
        message: format!(
            "Starting batch '{}' with {} video(s).",
            plan.batch_name,
            plan.items.len()
        ),
    }];
    let mut total_size = 0u64;
    for (index, result) in item_results.into_iter().enumerate() {
        let result = result.ok_or_else(|| {
            anyhow!(
                "Download worker for '{}' did not return a result.",
                plan.items[index].title
            )
        })?;
        log.extend(result.log_entries);
        if let ItemDownloadOutcome::Completed(size_bytes) = result.outcome {
            total_size = total_size.saturating_add(size_bytes);
        }
    }

    let channel = plan
        .items
        .first()
        .map(|item| sanitize_path_component(&item.channel));
    let playlist = plan.playlist_name.as_deref().map(sanitize_path_component);
    let record = Project::completed_download(
        plan.batch_name,
        plan.source_url,
        total_size,
        log,
        settings.download_root,
        channel,
        playlist,
        if plan.playlist_name.is_some() {
            MediaType::Playlist
        } else {
            MediaType::SingleVideo
        },
    )?;
    storage::save_history(&record)?;
    Ok(record)
}

pub fn download_concurrency_limit(plan: &DownloadPlan) -> usize {
    if plan.playlist_name.is_some() {
        MAX_CONCURRENT_PLAYLIST_ITEMS
    } else {
        1
    }
}

struct ItemDownloadResult {
    outcome: ItemDownloadOutcome,
    log_entries: Vec<DownloadLogEntry>,
}

enum ItemDownloadOutcome {
    Completed(u64),
    Aborted,
    Failed,
}

fn download_item_with_retries(
    index: usize,
    item: &MediaItem,
    plan: &DownloadPlan,
    settings: &Settings,
    events: &Sender<DownloadEvent>,
    abort_requests: Receiver<()>,
    total_attempts: u8,
) -> ItemDownloadResult {
    let mut log_entries = Vec::new();

    for attempt in 1..=total_attempts {
        if abort_requests.try_recv().is_ok() {
            log_entries.push(DownloadLogEntry::GenericMessage {
                message: format!("Aborted '{}'.", item.title),
            });
            let _ = events.send(DownloadEvent::ItemAborted { index });
            return ItemDownloadResult {
                outcome: ItemDownloadOutcome::Aborted,
                log_entries,
            };
        }
        let _ = events.send(DownloadEvent::AttemptStarted {
            index,
            attempt,
            total_attempts,
        });

        match download_item(index, item, plan, settings, events, &abort_requests) {
            Ok(Some((_, size_bytes))) => {
                log_entries.push(DownloadLogEntry::DownloadMessage {
                    message: format!("Downloaded '{}' ({} bytes).", item.title, size_bytes),
                });
                let _ = events.send(DownloadEvent::ItemCompleted { index, size_bytes });
                return ItemDownloadResult {
                    outcome: ItemDownloadOutcome::Completed(size_bytes),
                    log_entries,
                };
            }
            Ok(None) => {
                log_entries.push(DownloadLogEntry::GenericMessage {
                    message: format!("Aborted '{}'.", item.title),
                });
                let _ = events.send(DownloadEvent::ItemAborted { index });
                return ItemDownloadResult {
                    outcome: ItemDownloadOutcome::Aborted,
                    log_entries,
                };
            }
            Err(error) => {
                let message = format!("{error:#}");
                log_entries.push(DownloadLogEntry::GenericMessage {
                    message: format!(
                        "Attempt {attempt}/{total_attempts} failed for '{}': {message}",
                        item.title
                    ),
                });
                if attempt < total_attempts {
                    let _ = events.send(DownloadEvent::ItemFailed {
                        index,
                        message: format!(
                            "Attempt {attempt}/{total_attempts} failed; retrying: {message}"
                        ),
                    });
                } else {
                    log_entries.push(DownloadLogEntry::GenericMessage {
                        message: format!(
                            "Failed '{}' after {total_attempts} attempt(s): {message}",
                            item.title
                        ),
                    });
                    let _ = events.send(DownloadEvent::ItemFailed {
                        index,
                        message: format!("Failed after {total_attempts} attempt(s): {message}"),
                    });
                    return ItemDownloadResult {
                        outcome: ItemDownloadOutcome::Failed,
                        log_entries,
                    };
                }
            }
        }
    }

    unreachable!("download attempts are configured to run at least once")
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

pub fn file_type_args(settings: &Settings) -> &'static [&'static str] {
    match settings.file_type {
        FileType::FromSourceVideo => &[],
        FileType::Mp4 => &["--recode-video", "mp4"],
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
    abort_requests: &Receiver<()>,
) -> Result<Option<(PathBuf, u64)>> {
    let output_directory =
        download_output_directory(&settings.download_root, item, plan.playlist_name.as_deref());
    fs::create_dir_all(&output_directory).with_context(|| {
        format!(
            "Could not create download folder {}",
            output_directory.display()
        )
    })?;
    let temp_directory = if output_directory.is_absolute() {
        output_directory.clone()
    } else {
        std::env::current_dir()
            .context("Could not determine current folder.")?
            .join(&output_directory)
    };
    let output_template = temp_directory.join("%(title)s [%(id)s].%(ext)s");
    let mut child = tool_command::yt_dlp(&temp_directory)
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
        .args(file_type_args(settings))
        .arg("--output")
        .arg(output_template)
        .arg("--print")
        .arg(format!("after_move:{OUTPUT_MARKER}%(id)s|%(ext)s"))
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

    let stdout_events = events.clone();
    let stdout_reader = thread::spawn(move || {
        let mut output = Vec::new();
        let mut line = Vec::new();
        let mut reader = BufReader::new(stdout);
        loop {
            line.clear();
            let bytes_read = reader.read_until(b'\n', &mut line)?;
            if bytes_read == 0 {
                break;
            }
            if let Some(percent) = parse_progress_percent(&String::from_utf8_lossy(&line)) {
                let _ = stdout_events.send(DownloadEvent::Progress { index, percent });
            }
            output.extend_from_slice(&line);
        }
        Ok::<_, std::io::Error>(output)
    });

    let (status, aborted) = loop {
        if let Some(status) = child.try_wait().context("Could not check yt-dlp status.")? {
            break (status, false);
        }
        if abort_requests.try_recv().is_ok() {
            child
                .kill()
                .context("Could not stop yt-dlp after an abort request.")?;
            let status = child
                .wait()
                .context("Could not wait for aborted yt-dlp process.")?;
            break (status, true);
        }
        thread::sleep(Duration::from_millis(100));
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| anyhow!("Could not collect yt-dlp progress output."))?
        .context("Could not read yt-dlp progress output.")?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| anyhow!("Could not collect yt-dlp diagnostics."))?
        .context("Could not read yt-dlp diagnostics.")?;
    if aborted {
        return Ok(None);
    }
    if !status.success() {
        bail!("yt-dlp failed: {}", diagnostic_text(&stderr));
    }

    let _ = events.send(DownloadEvent::ItemVerifying { index });
    let output = String::from_utf8_lossy(&stdout);
    let (video_id, extension) = output
        .lines()
        .find_map(|line| line.trim().strip_prefix(OUTPUT_MARKER))
        .and_then(|details| details.split_once('|'))
        .ok_or_else(|| {
            anyhow!(
                "yt-dlp completed without reporting the video ID and extension for '{}'.",
                item.title
            )
        })?;
    let output_path = find_downloaded_file(&output_directory, video_id, extension)?;
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
    Ok(Some((output_path, metadata.len())))
}

pub fn find_downloaded_file(
    output_directory: &Path,
    video_id: &str,
    extension: &str,
) -> Result<PathBuf> {
    let suffix = format!("[{video_id}].{extension}");
    let mut matching_path = None;
    for entry in fs::read_dir(output_directory).with_context(|| {
        format!(
            "Could not list download folder {}.",
            output_directory.display()
        )
    })? {
        let entry = entry.with_context(|| {
            format!("Could not read an entry in {}.", output_directory.display())
        })?;
        if !entry
            .file_type()
            .with_context(|| format!("Could not inspect {}.", entry.path().display()))?
            .is_file()
        {
            continue;
        }
        if !entry.file_name().to_string_lossy().ends_with(&suffix) {
            continue;
        }
        if matching_path.replace(entry.path()).is_some() {
            bail!(
                "Multiple downloaded files matched video ID '{}' in {}.",
                video_id,
                output_directory.display()
            );
        }
    }
    matching_path.ok_or_else(|| {
        anyhow!(
            "Downloaded file with ID '{}' and extension '.{}' was not found in {}.",
            video_id,
            extension,
            output_directory.display()
        )
    })
}

fn verify_file(path: &Path) -> Result<()> {
    let output = tool_command::media_tool("ffmpeg")
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
    let output = tool_command::media_tool(program)
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

fn output_with_timeout(
    command: &mut Command,
    timeout: Duration,
    mut on_stderr_line: impl FnMut(&str),
) -> Result<Output> {
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
    let (stderr_lines_tx, stderr_lines_rx) = mpsc::channel();
    let stderr_reader = thread::spawn(move || {
        let mut output = Vec::new();
        let mut reader = BufReader::new(stderr);
        let mut line = Vec::new();
        loop {
            line.clear();
            let bytes_read = reader.read_until(b'\n', &mut line)?;
            if bytes_read == 0 {
                break;
            }
            output.extend_from_slice(&line);
            let _ = stderr_lines_tx.send(line.clone());
        }
        Ok(output)
    });
    let started = Instant::now();
    let status = loop {
        for line in stderr_lines_rx.try_iter() {
            on_stderr_line(&String::from_utf8_lossy(&line));
        }
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
    let stdout = join_command_reader(stdout_reader, "output")?;
    let stderr = join_command_reader(stderr_reader, "diagnostics")?;
    for line in stderr_lines_rx.try_iter() {
        on_stderr_line(&String::from_utf8_lossy(&line));
    }
    Ok(Output {
        status,
        stdout,
        stderr,
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
