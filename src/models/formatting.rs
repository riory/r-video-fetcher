use std::borrow::Cow;

pub(crate) fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

pub(crate) fn format_download_log_message(message: &str) -> Cow<'_, str> {
    let Some((title, bytes)) = message
        .strip_prefix("Downloaded '")
        .and_then(|text| text.strip_suffix(" bytes)."))
        .and_then(|text| text.rsplit_once("' ("))
    else {
        return Cow::Borrowed(message);
    };
    let Ok(bytes) = bytes.parse::<u64>() else {
        return Cow::Borrowed(message);
    };

    Cow::Owned(format!("Downloaded '{title}' ({}).", format_bytes(bytes)))
}
