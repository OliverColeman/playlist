pub mod models;

#[cfg(feature = "server")]
pub mod config;

#[cfg(feature = "server")]
pub mod database;

#[cfg(feature = "server")]
pub use config::Config;

#[cfg(feature = "server")]
pub use database::{ServerError, get_database};

// Re-exported so dependents can name timezone types (e.g. `chrono_tz::Tz`) without adding
// their own chrono-tz dependency.
pub use chrono_tz;

/// The canonical timezone in which playlist dates are interpreted and displayed.
///
/// The playlist sessions happen in Newcastle, Australia, so dates are always treated as being
/// in this zone — CLI `--date` input is parsed as midnight in this zone, and the web UI
/// formats timestamps in it — regardless of UTC or the host machine's local timezone.
pub const TIMEZONE: chrono_tz::Tz = chrono_tz::Tz::Australia__Sydney;

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
    let s = deunicode::deunicode(&s);
    let s = s.to_lowercase();
    let s = remove_punctuation(&s);
    remove_multiple_spaces_and_trim(&s)
}

pub fn normalise_name_strong(s: &str) -> String {
    let s = deunicode::deunicode(&s);
    let s = s.to_lowercase().trim().to_string();

    // Remove anything in brackets, except for brackets at the start
    let s = {
        let re_brackets = regex::Regex::new(r"(\([^)]*\)|\[[^\]]*\]|<[^>]*>)").unwrap();
        let mut chars = s.chars();
        // For an empty input there is no first char and this is the empty string.
        let first = chars.next().map(String::from).unwrap_or_default();
        let rest: String = chars.collect();
        let s = re_brackets.replace_all(&rest, "").to_string();
        format!("{}{}", first, s)
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

/** Returns an array containing the primary and secondary Double Metaphone codes for the given word.
 * Only strings normalised with [`normalise_name`] should be passed. */
pub fn generate_double_metaphone_codes(word: &str) -> Vec<String> {
    use rphonetic::{DoubleMetaphone, Encoder};
    let double_metaphone = DoubleMetaphone::default();
    let mut codes = Vec::new();
    // Catch panics from the rphonetic library
    let result = std::panic::catch_unwind(|| {
        let primary = double_metaphone.encode(word);
        let secondary = double_metaphone.encode_alternate(word);
        (primary, secondary)
    });

    match result {
        Ok((primary, secondary)) => {
            if !primary.is_empty() && !codes.contains(&primary) {
                codes.push(primary);
            }
            if !secondary.is_empty() && !codes.contains(&secondary) {
                codes.push(secondary);
            }
        }
        Err(error) => {
            println!(
                "Double Metaphone: failed for word: '{}'. Error: {:?}",
                word, error
            );
        }
    }
    codes
}

/** Generates n-grams from the given word.
 * Only strings normalised with [`normalise_name`] should be passed. */
pub fn generate_n_grams(word: &str) -> Vec<String> {
    static MIN_SIZE: usize = 2;
    static MAX_SIZE: usize = 3;
    let mut ngrams = Vec::new();
    let chars: Vec<char> = word.chars().collect();
    for n in MIN_SIZE..=MAX_SIZE {
        if chars.len() >= n {
            for i in 0..=(chars.len() - n) {
                let ngram: String = chars[i..i + n].iter().collect();
                if !ngrams.contains(&ngram) {
                    ngrams.push(ngram);
                }
            }
        }
    }
    ngrams
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- normalise_name ---

    #[test]
    fn normalise_name_lowercases() {
        assert_eq!(normalise_name("HELLO World"), "hello world");
    }

    #[test]
    fn normalise_name_deunicodes_accented_and_unicode_input() {
        assert_eq!(normalise_name("Café del Mar"), "cafe del mar");
        assert_eq!(normalise_name("Beyoncé"), "beyonce");
        assert_eq!(normalise_name("Björk"), "bjork");
        // CJK is transliterated by deunicode, then lowercased.
        assert_eq!(normalise_name("半分の月"), "ban fen noyue");
    }

    #[test]
    fn normalise_name_removes_quotes_without_inserting_spaces() {
        assert_eq!(normalise_name("Don't Stop Me Now"), "dont stop me now");
        assert_eq!(normalise_name("\"Quoted\" 'Name'"), "quoted name");
    }

    #[test]
    fn normalise_name_replaces_punctuation_classes_with_spaces() {
        assert_eq!(normalise_name("hello,world;foo:bar"), "hello world foo bar");
        assert_eq!(normalise_name("a\\b/c"), "a b c");
        assert_eq!(normalise_name("[Intro] <Test> {Curly}"), "intro test curly");
        assert_eq!(
            normalise_name("snake_case-and-hyphens"),
            "snake case and hyphens"
        );
    }

    #[test]
    fn normalise_name_collapses_multiple_spaces_and_trims() {
        // "/" and "-" become spaces, then runs of whitespace collapse to one space.
        assert_eq!(
            normalise_name("AC/DC - Back In Black"),
            "ac dc back in black"
        );
        assert_eq!(normalise_name("  a   b  "), "a b");
    }

    #[test]
    fn normalise_name_keeps_periods_and_other_symbols() {
        assert_eq!(normalise_name("  Mr.   Blue   Sky!  "), "mr. blue sky!");
        assert_eq!(normalise_name("R&B + Soul"), "r&b + soul");
    }

    #[test]
    fn normalise_name_empty_string() {
        assert_eq!(normalise_name(""), "");
    }

    // --- normalise_name_strong ---

    #[test]
    fn normalise_name_strong_removes_bracketed_content() {
        assert_eq!(
            normalise_name_strong("Song Title (Live at Wembley)"),
            "song title"
        );
        assert_eq!(normalise_name_strong("Song [Bonus Track]"), "song");
        assert_eq!(normalise_name_strong("Song <tag> here"), "song here");
    }

    #[test]
    fn normalise_name_strong_leading_bracket_protects_first_char() {
        // The first character is excluded from bracket matching, so a bracket that opens
        // at position 0 never matches and its content is kept (the bracket characters
        // themselves are later replaced with spaces by punctuation removal).
        assert_eq!(
            normalise_name_strong("(What's the Story) Morning Glory?"),
            "whats the story morning glory?"
        );
        // Only the first character is protected: a nested bracket after it still matches.
        assert_eq!(normalise_name_strong("((nested) outer)"), "outer");
    }

    #[test]
    fn normalise_name_strong_truncates_at_first_hyphen() {
        assert_eq!(normalise_name_strong("Song - 2011 Remaster"), "song");
        assert_eq!(normalise_name_strong("Song - Radio Edit"), "song");
        assert_eq!(normalise_name_strong("one-two-three"), "one");
    }

    #[test]
    fn normalise_name_strong_removes_feat_tail() {
        assert_eq!(normalise_name_strong("Song feat. Someone"), "song");
        assert_eq!(normalise_name_strong("Song feat Someone Else"), "song");
        // A bracketed feat credit is removed by the bracket rule.
        assert_eq!(normalise_name_strong("Song (feat. Guest)"), "song");
        // "featuring" is not matched by the feat rule.
        assert_eq!(
            normalise_name_strong("Song featuring Guest"),
            "song featuring guest"
        );
    }

    #[test]
    fn normalise_name_strong_removes_remastered_variants() {
        assert_eq!(
            normalise_name_strong("Song 2011 Remastered Version"),
            "song"
        );
        assert_eq!(normalise_name_strong("Song Digitally Remastered"), "song");
        assert_eq!(normalise_name_strong("Song Remastered 2011"), "song");
        assert_eq!(normalise_name_strong("Song Remaster"), "song");
    }

    #[test]
    fn normalise_name_strong_removes_remix_and_radio_edit_variants() {
        assert_eq!(normalise_name_strong("Song Remix"), "song");
        assert_eq!(normalise_name_strong("Song Radio Edit"), "song");
        assert_eq!(normalise_name_strong("Song Radio Mix"), "song");
        assert_eq!(normalise_name_strong("Song Radio Version"), "song");
    }

    #[test]
    fn normalise_name_strong_short_inputs_flow_through_the_full_pipeline() {
        // Short inputs go through the same pipeline as everything else; empty and
        // single-char inputs are handled safely by the bracket-stripping step.
        assert_eq!(normalise_name_strong("A"), "a");
        assert_eq!(normalise_name_strong("é"), "e");
        assert_eq!(normalise_name_strong(""), "");
        assert_eq!(normalise_name_strong(" "), "");
        // A lone hyphen is punctuation: replaced with a space, then trimmed away.
        assert_eq!(normalise_name_strong("-"), "");
    }

    #[test]
    fn normalise_name_strong_hyphen_inside_bracket_is_removed_with_the_bracket() {
        // Brackets are stripped before hyphen truncation, so the hyphen inside the
        // bracket does not truncate the rest of the name.
        assert_eq!(
            normalise_name_strong("Song (Live - Acoustic) Extra"),
            "song extra"
        );
    }

    // --- generate_n_grams ---

    #[test]
    fn generate_n_grams_of_words_shorter_than_two_chars_are_empty() {
        assert_eq!(generate_n_grams(""), Vec::<String>::new());
        assert_eq!(generate_n_grams("a"), Vec::<String>::new());
    }

    #[test]
    fn generate_n_grams_word_of_exactly_two_chars() {
        assert_eq!(generate_n_grams("ab"), vec!["ab"]);
    }

    #[test]
    fn generate_n_grams_word_of_exactly_three_chars() {
        assert_eq!(generate_n_grams("abc"), vec!["ab", "bc", "abc"]);
    }

    #[test]
    fn generate_n_grams_longer_word_lists_2_grams_then_3_grams() {
        assert_eq!(
            generate_n_grams("hello"),
            vec!["he", "el", "ll", "lo", "hel", "ell", "llo"]
        );
    }

    #[test]
    fn generate_n_grams_deduplicates_repeated_grams() {
        // "an", "na", "ana" and "nan" each occur more than once in "banana".
        assert_eq!(
            generate_n_grams("banana"),
            vec!["ba", "an", "na", "ban", "ana", "nan"]
        );
        assert_eq!(generate_n_grams("aaaa"), vec!["aa", "aaa"]);
    }

    #[test]
    fn generate_n_grams_counts_unicode_scalar_values_as_chars() {
        assert_eq!(generate_n_grams("日本語"), vec!["日本", "本語", "日本語"]);
    }

    // --- generate_double_metaphone_codes ---

    #[test]
    fn generate_double_metaphone_codes_primary_then_secondary_when_different() {
        assert_eq!(generate_double_metaphone_codes("smith"), vec!["SM0", "XMT"]);
        assert_eq!(
            generate_double_metaphone_codes("schmidt"),
            vec!["XMT", "SMT"]
        );
    }

    #[test]
    fn generate_double_metaphone_codes_deduplicates_identical_primary_and_secondary() {
        assert_eq!(generate_double_metaphone_codes("test"), vec!["TST"]);
        assert_eq!(generate_double_metaphone_codes("night"), vec!["NT"]);
        assert_eq!(generate_double_metaphone_codes("xylophone"), vec!["SLFN"]);
    }

    #[test]
    fn generate_double_metaphone_codes_empty_string_yields_no_codes() {
        assert_eq!(generate_double_metaphone_codes(""), Vec::<String>::new());
    }
}
