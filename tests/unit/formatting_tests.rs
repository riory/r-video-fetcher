#[path = "../../src/models/formatting.rs"]
mod formatting;

use formatting::format_download_log_message;

#[test]
fn formats_saved_download_sizes_for_display() {
    assert_eq!(
        format_download_log_message("Downloaded 'video' (1048576 bytes)."),
        "Downloaded 'video' (1.0 MB)."
    );
    assert_eq!(
        format_download_log_message("Downloaded 'title' (with parentheses)' (0 bytes)."),
        "Downloaded 'title' (with parentheses)' (0.0 B)."
    );
}

#[test]
fn preserves_other_download_log_messages() {
    for message in [
        "Downloaded test video.",
        "Downloaded 'video' (unknown bytes).",
        "Downloaded 'video' (18446744073709551616 bytes).",
        "Downloaded 'video' (1.0 MB).",
    ] {
        assert_eq!(format_download_log_message(message), message);
    }
}
