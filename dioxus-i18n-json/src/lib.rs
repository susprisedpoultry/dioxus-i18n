use dioxus::prelude::*;
use std::collections::HashMap;

pub mod config;
pub mod loader;

pub use config::I18nConfig;
pub use dioxus_i18n_json_macro::generate_keys;
pub use loader::{parse_translation_json, PluralForms, TranslationValue, Translations};

/// Handle to the i18n state.
///
/// This struct is `Copy` because it only holds [`Signal`]s, making it easy to
/// pass around in components and closures.
#[derive(Clone, Copy)]
pub struct I18n {
    locale: Signal<String>,
    fallback_locale: Signal<Option<String>>,
    translations: Signal<HashMap<String, Translations>>,
}

impl I18n {
    /// Translate a key into the current locale.
    ///
    /// If the key or locale is missing, the fallback locale is tried.
    /// If still missing, the key itself is returned.
    /// For plural values, the `other` fallback is returned.
    pub fn t(&self, key: &str) -> String {
        let translations = self.translations.read();
        let locale = self.locale.read();
        let fallback = self.fallback_locale.read();
        resolve_t(&translations, locale.as_str(), fallback.as_deref(), key)
    }

    /// Translate a key with pluralization.
    ///
    /// The translation file may contain a plural object (`one`, `other`, etc.).
    /// The correct form is selected using Unicode CLDR plural rules.
    /// The placeholder `{count}` is automatically replaced.
    /// Falls back to the configured fallback locale if the key is missing.
    pub fn tp(&self, key: &str, count: i64) -> String {
        let translations = self.translations.read();
        let locale = self.locale.read();
        let fallback = self.fallback_locale.read();
        resolve_tp(
            &translations,
            locale.as_str(),
            fallback.as_deref(),
            key,
            count,
        )
    }

    /// Translate a key and interpolate named placeholders.
    ///
    /// Placeholders in the template should be written as `{name}`.
    /// Falls back to the configured fallback locale if the key is missing.
    ///
    /// # Example
    /// ```rust,ignore
    /// let greeting = i18n.tf("greeting", &[("name", "Alice")]);
    /// ```
    pub fn tf(&self, key: &str, vars: &[(&str, &str)]) -> String {
        let translations = self.translations.read();
        let locale = self.locale.read();
        let fallback = self.fallback_locale.read();
        resolve_tf(
            &translations,
            locale.as_str(),
            fallback.as_deref(),
            key,
            vars,
        )
    }

    /// Switch the active locale.
    pub fn set_locale(&mut self, locale: &str) {
        self.locale.set(locale.to_string());
    }

    /// Get the current locale code.
    pub fn locale(&self) -> String {
        self.locale.read().clone()
    }

    /// Get the configured fallback locale code, if any.
    pub fn fallback_locale(&self) -> Option<String> {
        self.fallback_locale.read().clone()
    }

    /// Reload all translation files from disk.
    ///
    /// This is automatically called by the hot-reload watcher when the
    /// `hot-reload` feature is enabled.
    pub fn reload(&mut self, config: &I18nConfig) {
        match loader::load_translations(&config.locales_dir) {
            Ok(data) => {
                *self.translations.write() = data;
            }
            Err(e) => {
                tracing::error!("Failed to reload translations: {}", e);
            }
        }
    }
}

/// Pure helper: look up a raw translation value, trying fallback locale if configured.
fn resolve_value<'a>(
    translations: &'a HashMap<String, Translations>,
    locale: &str,
    fallback: Option<&str>,
    key: &str,
) -> Option<&'a TranslationValue> {
    let value = translations.get(locale).and_then(|t| t.get(key));
    if value.is_some() {
        return value;
    }
    if let Some(fb) = fallback {
        return translations.get(fb).and_then(|t| t.get(key));
    }
    None
}

/// Pure helper: translate a key, with fallback support.
fn resolve_t(
    translations: &HashMap<String, Translations>,
    locale: &str,
    fallback: Option<&str>,
    key: &str,
) -> String {
    match resolve_value(translations, locale, fallback, key) {
        Some(TranslationValue::String(s)) => s.clone(),
        Some(TranslationValue::Plural(pf)) => pf
            .other
            .clone()
            .or_else(|| {
                pf.one.clone().or_else(|| {
                    pf.few.clone().or_else(|| {
                        pf.many
                            .clone()
                            .or_else(|| pf.two.clone().or_else(|| pf.zero.clone()))
                    })
                })
            })
            .unwrap_or_else(|| key.to_string()),
        None => key.to_string(),
    }
}

/// Pure helper: translate a key with pluralization, with fallback support.
fn resolve_tp(
    translations: &HashMap<String, Translations>,
    locale: &str,
    fallback: Option<&str>,
    key: &str,
    count: i64,
) -> String {
    let value = resolve_value(translations, locale, fallback, key);

    let template = match value {
        Some(TranslationValue::String(s)) => Some(s.as_str()),
        Some(TranslationValue::Plural(pf)) => {
            // For fallback plurals, use the locale that actually provided the value
            let resolved_locale = if translations.get(locale).and_then(|t| t.get(key)).is_some() {
                locale
            } else {
                fallback.unwrap_or(locale)
            };
            let category = plural_category(resolved_locale, count);
            pf.get(category)
        }
        None => None,
    };

    match template {
        Some(s) => interpolate(s, &[("count", &count.to_string())]),
        None => key.to_string(),
    }
}

/// Pure helper: translate a key with interpolation, with fallback support.
fn resolve_tf(
    translations: &HashMap<String, Translations>,
    locale: &str,
    fallback: Option<&str>,
    key: &str,
    vars: &[(&str, &str)],
) -> String {
    let template = match resolve_value(translations, locale, fallback, key) {
        Some(TranslationValue::String(s)) => Some(s.as_str()),
        Some(TranslationValue::Plural(pf)) => pf.other.as_deref(),
        None => None,
    };

    match template {
        Some(s) => interpolate(s, vars),
        None => key.to_string(),
    }
}

fn plural_category(locale: &str, count: i64) -> &'static str {
    use icu_locale::Locale;
    use icu_plurals::{PluralCategory, PluralRuleType, PluralRules};

    let loc: Locale = locale.parse().unwrap_or(icu_locale::locale!("en"));
    let rules =
        PluralRules::try_new(loc.into(), PluralRuleType::Cardinal.into()).unwrap_or_else(|_| {
            PluralRules::try_new(
                icu_locale::locale!("en").into(),
                PluralRuleType::Cardinal.into(),
            )
            .expect("en data present")
        });

    match rules.category_for(count) {
        PluralCategory::Zero => "zero",
        PluralCategory::One => "one",
        PluralCategory::Two => "two",
        PluralCategory::Few => "few",
        PluralCategory::Many => "many",
        PluralCategory::Other => "other",
    }
}

fn interpolate(template: &str, vars: &[(&str, &str)]) -> String {
    let mut result = template.to_string();
    for (k, v) in vars {
        result = result.replace(&format!("{{{}}}", k), v);
    }
    result
}

/// Provide i18n context to the rest of the application.
///
/// # Example
/// ```rust,ignore
/// use dioxus_i18n_json::{I18nConfig, I18nProvider};
///
/// rsx! {
///     I18nProvider {
///         config: I18nConfig::new("./locales", "en"),
///         App {}
///     }
/// }
/// ```
#[component]
pub fn I18nProvider(children: Element, config: I18nConfig) -> Element {
    let initial = config
        .initial
        .clone()
        .or_else(|| loader::load_translations(&config.locales_dir).ok())
        .unwrap_or_default();
    let locale = use_signal(|| config.default_locale.clone());
    let fallback_locale = use_signal(|| config.fallback_locale.clone());
    let translations = use_signal(|| initial);

    let i18n = I18n {
        locale,
        fallback_locale,
        translations,
    };
    use_context_provider(|| i18n);

    #[cfg(feature = "hot-reload")]
    {
        let mut i18n = i18n;
        let cfg = config.clone();
        use_hook(|| {
            let mut rx = hot_reload::watch_translations(cfg.locales_dir.clone());
            spawn(async move {
                while rx.recv().await.is_some() {
                    i18n.reload(&cfg);
                }
            });
        });
    }

    rsx! { {children} }
}

/// Access the i18n handle from the current context.
///
/// Must be called inside a component tree wrapped by [`I18nProvider`].
pub fn use_i18n() -> I18n {
    use_context::<I18n>()
}

/// A convenience handle returned by [`use_t`] that exposes translation helpers
/// in a destructurable struct.
///
/// # Example
/// ```rust,ignore
/// let UseI18n { t, tp, tf } = use_t();
/// let msg = t("greeting");
/// let count = tp("itemCount", 5);
/// let hello = tf("hello", &[("name", "Alice")]);
/// ```
#[derive(Clone, Copy)]
pub struct UseI18n<T, TP, TF>
where
    T: Fn(&str) -> String + Copy,
    TP: Fn(&str, i64) -> String + Copy,
    TF: Fn(&str, &[(&str, &str)]) -> String + Copy,
{
    pub t: T,
    pub tp: TP,
    pub tf: TF,
}

/// Returns callable translation helpers.
///
/// # Example
/// ```rust,ignore
/// let UseI18n { t, tp, tf } = use_t();
/// let welcome = t("messages.welcome");
/// ```
#[allow(clippy::type_complexity)]
pub fn use_t() -> UseI18n<
    impl Fn(&str) -> String + Copy,
    impl Fn(&str, i64) -> String + Copy,
    impl Fn(&str, &[(&str, &str)]) -> String + Copy,
> {
    let i18n = use_context::<I18n>();
    UseI18n {
        t: move |key| i18n.t(key),
        tp: move |key, count| i18n.tp(key, count),
        tf: move |key, vars| i18n.tf(key, vars),
    }
}

/// Returns callable translation helpers bound to a namespace.
///
/// The namespace is prepended to every key automatically.
///
/// # Example
/// ```rust,ignore
/// let UseI18n { t, tp, tf } = use_t_ns("messages");
/// let welcome = t("welcome"); // resolves to "messages.welcome"
/// ```
#[allow(clippy::type_complexity)]
pub fn use_t_ns(
    ns: &'static str,
) -> UseI18n<
    impl Fn(&str) -> String + Copy,
    impl Fn(&str, i64) -> String + Copy,
    impl Fn(&str, &[(&str, &str)]) -> String + Copy,
> {
    let i18n = use_context::<I18n>();
    UseI18n {
        t: move |key| i18n.t(&format!("{}.{}", ns, key)),
        tp: move |key, count| i18n.tp(&format!("{}.{}", ns, key), count),
        tf: move |key, vars| i18n.tf(&format!("{}.{}", ns, key), vars),
    }
}

/// Render a translation string that contains component placeholders.
///
/// Placeholders look like `<0>...</0>`, `<1>...</1>`, etc. The text between
/// the tags is ignored; the matching slot from `slots` is rendered instead.
///
/// # Example
/// ```rust,ignore
/// Trans {
///     text: t("terms"),
///     slots: vec![
///         rsx! { a { href: "/tos", "Terms of Service" } }
///     ]
/// }
/// ```
#[component]
pub fn Trans(text: String, slots: Vec<Element>) -> Element {
    let mut parts: Vec<Element> = Vec::new();
    let mut rest = text.as_str();

    while let Some(start) = rest.find('<') {
        if start > 0 {
            let before = rest[..start].to_string();
            parts.push(rsx! { {before} });
        }

        let after = &rest[start + 1..];
        if let Some(end) = after.find('>') {
            let tag = &after[..end];
            let after_tag = &after[end + 1..];
            let close = format!("</{}>", tag);
            if let Some(close_idx) = after_tag.find(&close) {
                let inner = &after_tag[..close_idx];
                if let Ok(idx) = tag.parse::<usize>() {
                    if let Some(slot) = slots.get(idx) {
                        parts.push(slot.clone());
                    } else {
                        parts.push(rsx! { {inner.to_string()} });
                    }
                } else {
                    parts.push(rsx! { {inner.to_string()} });
                }
                rest = &after_tag[close_idx + close.len()..];
                continue;
            }
        }

        // Malformed tag – treat the '<' as literal text
        parts.push(rsx! { "<" });
        rest = &rest[start + 1..];
    }

    if !rest.is_empty() {
        parts.push(rsx! { {rest.to_string()} });
    }

    rsx! { for part in parts { {part} } }
}

#[cfg(feature = "hot-reload")]
mod hot_reload;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolate() {
        assert_eq!(
            interpolate("Hello, {name}!", &[("name", "Alice")]),
            "Hello, Alice!"
        );
        assert_eq!(
            interpolate("{a} and {b}", &[("a", "1"), ("b", "2")]),
            "1 and 2"
        );
        assert_eq!(interpolate("No placeholders", &[]), "No placeholders");
    }

    #[test]
    fn test_plural_category_english() {
        assert_eq!(plural_category("en", 1), "one");
        assert_eq!(plural_category("en", 0), "other");
        assert_eq!(plural_category("en", 5), "other");
        assert_eq!(plural_category("en", 11), "other");
    }

    #[test]
    fn test_plural_category_russian() {
        // Russian: one → 1, 21, 31...; few → 2-4, 22-24...; many → 0, 5-20, 25-30...
        assert_eq!(plural_category("ru", 1), "one");
        assert_eq!(plural_category("ru", 2), "few");
        assert_eq!(plural_category("ru", 5), "many");
        assert_eq!(plural_category("ru", 11), "many");
        assert_eq!(plural_category("ru", 21), "one");
        assert_eq!(plural_category("ru", 22), "few");
    }

    #[test]
    fn test_parse_translation_json_flattening() {
        let json = r#"{"greeting":"Hello","messages":{"welcome":"Welcome"}}"#;
        let map = parse_translation_json(json).unwrap();
        assert_eq!(
            map.get("greeting"),
            Some(&TranslationValue::String("Hello".to_string()))
        );
        assert_eq!(
            map.get("messages.welcome"),
            Some(&TranslationValue::String("Welcome".to_string()))
        );
    }

    #[test]
    fn test_parse_translation_json_plural_object() {
        let json = r#"{"itemCount":{"one":"1 item","other":"{count} items"}}"#;
        let map = parse_translation_json(json).unwrap();
        match map.get("itemCount") {
            Some(TranslationValue::Plural(pf)) => {
                assert_eq!(pf.one, Some("1 item".to_string()));
                assert_eq!(pf.other, Some("{count} items".to_string()));
            }
            other => panic!("expected plural form, got {:?}", other),
        }
    }

    #[test]
    fn test_plural_forms_get() {
        let pf = PluralForms {
            one: Some("one item".to_string()),
            other: Some("{count} items".to_string()),
            ..Default::default()
        };

        assert_eq!(pf.get("one"), Some("one item"));
        assert_eq!(pf.get("other"), Some("{count} items"));
        assert_eq!(pf.get("few"), Some("{count} items")); // falls back to other
    }

    #[test]
    fn test_fallback_locale_t() {
        let mut translations = HashMap::new();
        let mut en = Translations::new();
        en.insert(
            "greeting".to_string(),
            TranslationValue::String("Hello".to_string()),
        );
        en.insert(
            "missing_in_es".to_string(),
            TranslationValue::String("Fallback".to_string()),
        );

        let mut es = Translations::new();
        es.insert(
            "greeting".to_string(),
            TranslationValue::String("Hola".to_string()),
        );

        translations.insert("en".to_string(), en);
        translations.insert("es".to_string(), es);

        assert_eq!(
            resolve_t(&translations, "es", Some("en"), "greeting"),
            "Hola"
        );
        assert_eq!(
            resolve_t(&translations, "es", Some("en"), "missing_in_es"),
            "Fallback"
        );
        assert_eq!(
            resolve_t(&translations, "es", Some("en"), "missing_everywhere"),
            "missing_everywhere"
        );
        // no fallback configured
        assert_eq!(
            resolve_t(&translations, "es", None, "missing_in_es"),
            "missing_in_es"
        );
    }

    #[test]
    fn test_fallback_locale_tf() {
        let mut translations = HashMap::new();
        let mut en = Translations::new();
        en.insert(
            "hello".to_string(),
            TranslationValue::String("Hello, {name}".to_string()),
        );

        let mut es = Translations::new();
        es.insert(
            "greeting".to_string(),
            TranslationValue::String("Hola".to_string()),
        );

        translations.insert("en".to_string(), en);
        translations.insert("es".to_string(), es);

        assert_eq!(
            resolve_tf(
                &translations,
                "es",
                Some("en"),
                "hello",
                &[("name", "Alice")]
            ),
            "Hello, Alice"
        );
        assert_eq!(
            resolve_tf(&translations, "es", Some("en"), "missing_everywhere", &[]),
            "missing_everywhere"
        );
    }

    #[test]
    fn test_fallback_locale_tp() {
        let mut translations = HashMap::new();
        let en = PluralForms {
            one: Some("1 item".to_string()),
            other: Some("{count} items".to_string()),
            ..Default::default()
        };
        let mut en_map = Translations::new();
        en_map.insert("itemCount".to_string(), TranslationValue::Plural(en));

        translations.insert("en".to_string(), en_map);
        translations.insert("es".to_string(), Translations::new());

        assert_eq!(
            resolve_tp(&translations, "es", Some("en"), "itemCount", 1),
            "1 item"
        );
        assert_eq!(
            resolve_tp(&translations, "es", Some("en"), "itemCount", 5),
            "5 items"
        );
        assert_eq!(
            resolve_tp(&translations, "es", Some("en"), "missing_everywhere", 1),
            "missing_everywhere"
        );
    }
}
