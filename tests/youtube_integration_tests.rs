use r_video_fetcher::{
    downloader::{DownloadPlan, download_batch, download_output_directory, inspect_batch},
    model::Settings,
};
use std::{
    collections::HashSet,
    fs,
    sync::mpsc,
    time::{SystemTime, UNIX_EPOCH},
};
use url::Url;

const PLAYLIST_URL: &str =
    "https://www.youtube.com/watch?v=m4hjb4d8ri0&list=PLUiUn_cZl605Fpznit1AyT_c6d993P68K";
const VIDEO_URL: &str = "https://www.youtube.com/watch?v=GpcaJQ40q1Y";

fn settings_for(test_name: &str) -> Settings {
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|error| panic!("System clock is earlier than the Unix epoch: {error}"))
        .as_nanos();
    let settings = Settings {
        download_root: std::env::current_dir()
            .unwrap_or_else(|error| panic!("Could not determine project folder: {error}"))
            .join("download-test")
            .join(format!("{test_name}-{}-{run_id}", std::process::id())),
        ..Settings::default()
    };
    settings
}

fn inspect(url: &str, settings: &Settings) -> DownloadPlan {
    inspect_batch(url, settings)
        .unwrap_or_else(|error| panic!("Could not inspect {url}: {error:#}"))
}

fn assert_http_url(url: &str) {
    let parsed = Url::parse(url).unwrap_or_else(|error| panic!("Invalid URL {url}: {error}"));
    assert!(
        matches!(parsed.scheme(), "http" | "https"),
        "invalid URL: {url}"
    );
    assert!(parsed.host_str().is_some(), "URL has no host: {url}");
}

fn assert_plan_urls_are_valid(plan: &DownloadPlan) {
    assert_http_url(&plan.source_url);
    for item in &plan.items {
        assert_http_url(&item.url);
    }
}

fn download_and_assert_files(plan: DownloadPlan, settings: Settings) {
    let expected_file_count = plan.items.len();
    let output_directories = plan
        .items
        .iter()
        .map(|item| {
            download_output_directory(&settings.download_root, item, plan.playlist_name.as_deref())
        })
        .collect::<HashSet<_>>();
    let (events, _receiver) = mpsc::channel();
    let record = download_batch(plan, settings, events)
        .unwrap_or_else(|error| panic!("Could not download inspected media: {error:#}"));

    assert!(record.size_bytes > 0, "downloaded media must not be empty");
    let downloaded_file_count = output_directories
        .iter()
        .map(|directory| {
            let entries = fs::read_dir(directory).unwrap_or_else(|error| {
                panic!(
                    "Could not read runtime-derived output directory {}: {error}",
                    directory.display()
                )
            });
            entries
                .map(|entry| {
                    let entry = entry.unwrap_or_else(|error| {
                        panic!(
                            "Could not read an entry in {}: {error}",
                            directory.display()
                        )
                    });
                    let metadata = entry.metadata().unwrap_or_else(|error| {
                        panic!(
                            "Could not inspect downloaded file {}: {error}",
                            entry.path().display()
                        )
                    });
                    usize::from(metadata.is_file() && metadata.len() > 0)
                })
                .sum::<usize>()
        })
        .sum::<usize>();
    assert!(
        downloaded_file_count >= expected_file_count,
        "expected at least {expected_file_count} non-empty downloaded file(s), found {downloaded_file_count}"
    );
}

#[test]
#[ignore = "requires network access and yt-dlp plus ffmpeg on PATH"]
fn downloads_the_first_two_videos_from_the_specified_50_video_playlist() {
    let settings = settings_for("playlist");
    let mut plan = inspect(PLAYLIST_URL, &settings);

    assert_plan_urls_are_valid(&plan);
    assert_eq!(plan.items.len(), 50, "playlist should contain 50 videos");
    assert_eq!(plan.batch_name, "Smiles 4 Grade 4 Pupil’s book");
    assert_eq!(
        plan.playlist_name.as_deref(),
        Some("Smiles 4 Grade 4 Pupil’s book")
    );
    assert_eq!(
        plan.unknown_size_count, 0,
        "yt-dlp should provide a size estimate for every playlist video"
    );
    assert!(plan.items.len() >= 2);
    plan.items.truncate(2);
    download_and_assert_files(plan, settings);
}

#[test]
#[ignore = "requires network access and yt-dlp plus ffmpeg on PATH"]
fn downloads_the_rainy_city_night_video() {
    let settings = settings_for("single-video");
    let plan = inspect(VIDEO_URL, &settings);

    assert_plan_urls_are_valid(&plan);
    assert_eq!(
        plan.items.len(),
        1,
        "single-video URL should contain one video"
    );
    assert_eq!(
        plan.items[0].title,
        "Rainy City Night - Royalty Free Stock Footage"
    );
    download_and_assert_files(plan, settings);
}
