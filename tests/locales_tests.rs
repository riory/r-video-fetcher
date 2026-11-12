use r_video_fetcher::{
    locales::{Label, LocaleStrings},
    model::Language,
};

#[test]
fn locale_files_load_all_required_resources() {
    let locales = LocaleStrings::load().unwrap();

    assert_eq!(locales.text(Language::English, Label::Exit), "Exit");
    assert_eq!(locales.text(Language::Russian, Label::Exit), "Выход");
    assert_eq!(
        locales.text(Language::Russian, Label::Resolution1080),
        "1080p"
    );
}
