use std::collections::HashMap;
use std::sync::OnceLock;

static DICTS: OnceLock<HashMap<String, HashMap<String, String>>> = OnceLock::new();

fn load_dicts() -> &'static HashMap<String, HashMap<String, String>> {
    DICTS.get_or_init(|| {
        let mut dicts = HashMap::new();
        dicts.insert(
            "zh-CN".to_string(),
            parse_locale(include_str!("../../locales/zh-CN.toml")),
        );
        dicts.insert(
            "en-US".to_string(),
            parse_locale(include_str!("../../locales/en-US.toml")),
        );
        dicts
    })
}

fn parse_locale(content: &str) -> HashMap<String, String> {
    let value: toml::Value = content
        .parse()
        .unwrap_or_else(|err| panic!("invalid locale toml: {err}"));
    let mut out = HashMap::new();
    flatten_toml("", &value, &mut out);
    out
}

fn flatten_toml(prefix: &str, value: &toml::Value, out: &mut HashMap<String, String>) {
    match value {
        toml::Value::Table(table) => {
            for (key, nested) in table {
                let full_key = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten_toml(&full_key, nested, out);
            }
        }
        toml::Value::String(text) => {
            out.insert(prefix.to_string(), text.clone());
        }
        _ => {}
    }
}

pub fn resolve_locale(site_language: &str) -> &'static str {
    match site_language.trim() {
        "en-US" | "en" | "en-us" => "en-US",
        _ => "zh-CN",
    }
}

pub fn translate(key: &str, locale: &str) -> String {
    let locale = resolve_locale(locale);
    let dicts = load_dicts();
    if let Some(text) = dicts.get(locale).and_then(|dict| dict.get(key)) {
        return text.clone();
    }
    if locale != "zh-CN" {
        if let Some(text) = dicts.get("zh-CN").and_then(|dict| dict.get(key)) {
            return text.clone();
        }
    }
    key.to_string()
}

pub fn translate_with(key: &str, locale: &str, args: &[&str]) -> String {
    let mut text = translate(key, locale);
    for (index, arg) in args.iter().enumerate() {
        text = text.replace(&format!("{{{index}}}"), arg);
    }
    text
}

pub fn locale_keys(locale: &str) -> Vec<String> {
    let locale = resolve_locale(locale);
    let mut keys = load_dicts()
        .get(locale)
        .map(|dict| dict.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    keys.sort();
    keys
}

pub fn locales_have_same_keys() -> bool {
    let dicts = load_dicts();
    let zh = dicts.get("zh-CN").map(|dict| dict.keys().collect::<Vec<_>>());
    let en = dicts.get("en-US").map(|dict| dict.keys().collect::<Vec<_>>());
    match (zh, en) {
        (Some(mut zh_keys), Some(mut en_keys)) => {
            zh_keys.sort();
            en_keys.sort();
            zh_keys == en_keys
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zh_and_en_locale_keys_match() {
        assert!(locales_have_same_keys());
    }

    #[test]
    fn translate_falls_back_to_zh_cn() {
        assert_eq!(translate("nav.home", "en-US"), "Home");
        assert_eq!(translate("nav.home", "zh-CN"), "首页");
    }

    #[test]
    fn translate_with_replaces_placeholders() {
        let text = translate_with("error.field_required", "zh-CN", &["邮箱"]);
        assert_eq!(text, "邮箱不能为空");
    }
}
