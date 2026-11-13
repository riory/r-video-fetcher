use r_video_fetcher::{
    models::locales::{Label, LocaleStrings},
    models::structs::Language,
};
use std::collections::BTreeMap;

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

#[test]
fn russian_resources_include_translations_and_preserve_placeholders() {
    let locales = LocaleStrings::load().unwrap();
    assert_eq!(locales.text(Language::Russian, Label::Exit), "Выход");
    assert_eq!(locales.text(Language::Russian, Label::Russian), "Русский");
    assert_eq!(locales.text(Language::English, Label::Russian), "Russian");

    let english: BTreeMap<String, String> =
        serde_json::from_str(include_str!("../../src/locales/en.json")).unwrap();
    let russian: BTreeMap<String, String> =
        serde_json::from_str(include_str!("../../src/locales/ru.json")).unwrap();
    assert_eq!(
        english.keys().collect::<Vec<_>>(),
        russian.keys().collect::<Vec<_>>()
    );
    for (key, value) in english {
        let placeholders = |text: &str| {
            let mut values: Vec<String> = text
                .split('{')
                .skip(1)
                .filter_map(|part| part.split_once('}').map(|(name, _)| name.to_owned()))
                .collect();
            values.sort();
            values
        };
        assert_eq!(
            placeholders(&value),
            placeholders(&russian[&key]),
            "placeholder mismatch for {key}"
        );
    }
}
