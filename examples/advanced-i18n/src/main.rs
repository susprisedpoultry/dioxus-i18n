use dioxus::prelude::*;
use dioxus_i18n_json::{
    generate_keys, use_i18n, use_i18n_provider, use_t, I18nConfig, Tr, UseI18n,
};

generate_keys!("examples/advanced-i18n/locales/en.json");

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    use_i18n_provider(
        I18nConfig::new("examples/advanced-i18n/locales", "en").with_fallback_locale("en"),
    );

    rsx! {
        Dashboard {}
    }
}

#[component]
fn Dashboard() -> Element {
    let mut i18n = use_i18n();
    let UseI18n { t, tp, tf } = use_t();

    // Stateful counters for plural demos
    let mut unread = use_signal(|| 1i64);
    let mut followers = use_signal(|| 42i64);

    let locale = i18n.locale();

    rsx! {
        div {
            style: "font-family: sans-serif; padding: 2rem; max-width: 700px; margin: 0 auto; line-height: 1.6;",

            // Header with Trans wrapping multiple inline components
            h1 { {t(keys::app::title)} }
            Trans {
                text: t(keys::app::subtitle),
                slots: vec![
                    rsx! { a { href: "https://github.com/yourusername/dioxus-i18n", style: "color: #0969da; text-decoration: none;", "dioxus-i18n" } },
                    rsx! { a { href: "https://www.rust-lang.org", style: "color: #ce422b; text-decoration: none;", "Rust" } }
                ]
            }

            hr {}

            // Profile greeting with formatted variables
            p {
                style: "font-size: 1.1rem;",
                {tf(keys::profile::greeting, &[("name", "Alex"), ("date", "2024-05-12")])}
            }

            // Stats with pluralization
            div {
                style: "display: flex; gap: 2rem; margin: 1rem 0;",
                div {
                    button { onclick: move |_| followers -= 1, "−" }
                    span { " {followers} " }
                    button { onclick: move |_| followers += 1, "+" }
                    p { {tp(keys::profile::stats::followers, followers())} }
                }
            }

            hr {}

            // Notifications with pluralization + component wrapper
            h2 { {t(keys::notifications::title)} }
            div {
                style: "display: flex; align-items: center; gap: 1rem;",
                button { onclick: move |_| unread -= 1, "−" }
                span { " {unread} " }
                button { onclick: move |_| unread += 1, "+" }
            }
            p {
                Trans {
                    text: tp(keys::notifications::unread, unread()),
                    slots: vec![
                        rsx! { strong { style: "color: red;", "" } }
                    ]
                }
            }

            hr {}

            // CTA buttons
            div {
                style: "display: flex; gap: 1rem;",
                button { {t(keys::cta::save)} }
                button { {t(keys::cta::discard)} }
            }

            hr {}

            // Footer with Trans + formatted year
            p {
                Trans {
                    text: tf(keys::footer::copyright, &[("year", "2026")]),
                    slots: vec![
                        rsx! { a { href: "#", "My Company" } }
                    ]
                }
            }
            div {
                style: "display: flex; gap: 1rem; font-size: 0.9rem;",
                a { href: "#", {t(keys::footer::links::privacy)} }
                a { href: "#", {t(keys::footer::links::terms)} }
            }

            hr {}

            // Locale switcher
            div {
                style: "display: flex; gap: 1rem; margin-top: 1rem;",
                button {
                    disabled: locale == "en",
                    onclick: move |_| i18n.set_locale("en"),
                    "🇬🇧 English"
                }
                button {
                    disabled: locale == "ru",
                    onclick: move |_| i18n.set_locale("ru"),
                    "🇷🇺 Русский"
                }
                button {
                    disabled: locale == "pl",
                    onclick: move |_| i18n.set_locale("pl"),
                    "🇵🇱 Polski"
                }
            }
        }
    }
}
