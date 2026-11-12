use r_video_fetcher::{
    model::{HistoryRecord, MediaType, Settings},
    storage::{remember_recent_file, write_json},
};
use std::{
    env, fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn writes_and_reads_json_history_files() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = env::temp_dir().join(format!("r-video-fetcher-test-{unique}"));
    let path = directory.join("batch.dwnl");
    let expected = HistoryRecord {
        batch_name: "Test batch".to_owned(),
        url: "https://example.com/video".to_owned(),
        created_at_unix_seconds: 1,
        size_bytes: 42,
        download_log: vec!["Downloaded test video".to_owned()],
        target_folder: PathBuf::from("downloads"),
        media_type: Some(MediaType::SingleVideo),
    };

    write_json(&path, &expected).unwrap();
    let actual: HistoryRecord = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();

    assert_eq!(actual.batch_name, expected.batch_name);
    assert_eq!(actual.url, expected.url);
    assert_eq!(actual.size_bytes, expected.size_bytes);
    assert_eq!(actual.download_log, expected.download_log);
    assert_eq!(actual.target_folder, expected.target_folder);
    assert_eq!(actual.media_type, expected.media_type);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn loads_legacy_project_files_without_new_optional_fields() {
    let project: HistoryRecord = serde_json::from_str(
        r#"{
            "batch_name": "Legacy",
            "url": "https://example.com/video",
            "created_at_unix_seconds": 1,
            "size_bytes": 0,
            "download_log": []
        }"#,
    )
    .unwrap();

    assert!(project.target_folder.as_os_str().is_empty());
    assert_eq!(project.media_type, None);
}

#[test]
fn recent_projects_are_deduplicated_and_limited_to_ten() {
    let mut settings = Settings::default();
    let paths = (0..11)
        .map(|index| PathBuf::from(format!("project-{index}.dwnl")))
        .collect::<Vec<_>>();
    for path in &paths {
        remember_recent_file(&mut settings, path);
    }
    remember_recent_file(&mut settings, &paths[5]);

    assert_eq!(settings.recent_files.len(), 10);
    assert_eq!(settings.recent_files[0], paths[5]);
    assert!(!settings.recent_files.contains(&paths[0]));
}

#[test]
fn settings_without_recent_projects_keep_compatibility() {
    let settings: Settings =
        serde_json::from_str(r#"{"download_root":"downloads","max_retries":3}"#).unwrap();

    assert!(settings.recent_files.is_empty());
    assert_eq!(settings.max_retries, 3);
}
