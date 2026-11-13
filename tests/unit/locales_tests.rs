use r_video_fetcher::{
    locales::{Label, LocaleStrings},
    model::Language,
};

#[test]
fn locale_files_load_all_required_resources() {
    let locales = LocaleStrings::load().unwrap();

    assert_eq!(locales.text(Language::English, Label::Exit), "Exit");
    assert_eq!(
        locales.text(Language::English, Label::Resolution1080),
        "1080p"
    );
    assert_eq!(
        locales.text(Language::English, Label::Language),
        "UI language"
    );
    assert_eq!(
        locales.text(Language::English, Label::FromSourceVideo),
        "From source video"
    );
    assert_eq!(
        locales.text(Language::English, Label::EstimatingItems),
        "Estimating sizes: {completed} of {total} complete ({active} active, {unavailable} unavailable)…"
    );
}
