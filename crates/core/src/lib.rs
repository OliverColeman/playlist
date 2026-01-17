pub mod models;

#[cfg(feature = "server")]
pub mod config;

#[cfg(feature = "server")]
pub mod database;

#[cfg(feature = "server")]
pub use config::Config;

#[cfg(feature = "server")]
pub use database::{ServerError, get_database};

/** Removes punctuation. This must be i18n compatible so don't enforce alphanumeric here.
Most symbols, including periods, are kept as they may appear in names. */
fn remove_punctuation(s: &str) -> String {
    // No space.
    let s = s.replace(&['\'', '"'][..], "");
    // Replace with a space.
    let re_punct = regex::Regex::new(r"[\/\\()\[\]{}<>\-_;:,]").unwrap();
    re_punct.replace_all(&s, " ").to_string()
}

fn remove_multiple_spaces_and_trim(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
        .trim()
        .to_string()
}

pub fn normalise_name(s: &str) -> String {
    let s = s.to_lowercase();
    let s = remove_punctuation(&s);
    remove_multiple_spaces_and_trim(&s)
}

pub fn normalise_name_strong(s: &str) -> String {
    let s = s.to_lowercase().trim().to_string();
    // Remove anything in brackets, except for brackets at the start
    let s = {
        let re_brackets = regex::Regex::new(r"(\([^)]*\)|\[[^\]]*\]|<[^>]*>)").unwrap();
        if s.len() < 2 {
            return s;
        }
        let mut chars = s.chars();
        let first = chars.next();
        let rest: String = chars.collect();
        let s = re_brackets.replace_all(&rest, "").to_string();
        format!("{}{}", first.unwrap_or_default(), s)
    };
    // Remove anything after the first hyphen
    let s = {
        let re_hyphen = regex::Regex::new(r"^([^-]+)-.*$").unwrap();
        re_hyphen.replace(&s, "$1").to_string()
    };
    // Remove anything after feat or feat.
    let s = {
        let re_feat = regex::Regex::new(r"^(.*?)(\s+feat\.?\s.*)$").unwrap();
        re_feat.replace(&s, "$1").to_string()
    };

    // Remove punctuation. This must be i18n compatible so don't enforce alphanumeric here.
    // Most symbols, including periods, are kept as they may appear in names.
    let s = remove_punctuation(&s);

    // remove "remastered" and similar
    let s = {
        let re_remaster = regex::Regex::new(
            r"(\s\d{4,4})?\s(digital(ly)?\s)?remaster(ed)?(\sversion)?(\s\d{4,4})?",
        )
        .unwrap();
        re_remaster.replace_all(&s, "").to_string()
    };
    // Remove 'remix', 'radio edit' and similar
    let s = {
        let re_radio_edit =
            regex::Regex::new(r"(\s+(remix|radio\s+(edit|cut|mix|version))\s*)").unwrap();
        re_radio_edit.replace_all(&s, "").to_string()
    };

    remove_multiple_spaces_and_trim(&s)
}
