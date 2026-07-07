//! Integration checks for embedded frontend locale dictionaries.

#[test]
fn locale_files_have_matching_keys() {
    assert!(free_market::services::i18n_service::locales_have_same_keys());
}

#[test]
fn common_frontend_keys_translate_in_english() {
    let keys = [
        "nav.home",
        "nav.order_search",
        "buy.order_now",
        "search.title",
        "error.out_of_stock",
        "error.invalid_email",
    ];
    for key in keys {
        let text = free_market::services::i18n_service::translate(key, "en-US");
        assert_ne!(text, key, "missing en-US translation for {key}");
    }
}
