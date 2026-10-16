//! The analyzer under test: what a document's text and a query's both become
//! before the index sees them. The same text always gives the same tokens,
//! and which stemmer a word meets is chosen by its script, never by guessing
//! its language, so that a two-word query and a long document agree.
//!
//! ```text
//! "Кари́бский кризис, café 世界和平"
//!   NFKC, lower case, stress marks off, ё → е, Latin accents folded
//!   words by Unicode's rules            → карибский | кризис | cafe
//!   a run of Han, kana or Thai          → 世界 | 界和 | 和平   (overlapping bigrams)
//!   a stem where the script has one     → карибск | кризис | cafe
//! ```

use rust_stemmers::{Algorithm, Stemmer};
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;
use unicode_segmentation::UnicodeSegmentation;

/// How a run of a script written without spaces becomes tokens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Segment {
    /// Overlapping pairs of characters, no dictionary.
    Bigrams,
    /// ICU4X's word segmenter: dictionaries for Chinese and Japanese, LSTM for Thai.
    #[cfg(feature = "icu")]
    Icu,
    /// Unicode's word rules alone, as SQLite's unicode61 would see them.
    Words,
}

/// An analysis: one name in the report, one way of making tokens.
#[derive(Clone, Copy, Debug)]
pub struct Analysis {
    pub segment: Segment,
    /// The stemmer for Latin script; Cyrillic always takes Russian's.
    pub latin: Option<Algorithm>,
    pub stem_cyrillic: bool,
    pub fold_latin: bool,
}

impl Analysis {
    pub fn named(name: &str) -> Analysis {
        let base = Analysis {
            segment: Segment::Bigrams,
            latin: Some(Algorithm::English),
            stem_cyrillic: true,
            fold_latin: true,
        };
        match name {
            "auto" => base,
            "surface" => Analysis {
                latin: None,
                stem_cyrillic: false,
                ..base
            },
            "words" => Analysis {
                segment: Segment::Words,
                ..base
            },
            #[cfg(feature = "icu")]
            "icu" => Analysis {
                segment: Segment::Icu,
                ..base
            },
            "de" => Analysis {
                latin: Some(Algorithm::German),
                ..base
            },
            "fr" => Analysis {
                latin: Some(Algorithm::French),
                ..base
            },
            "es" => Analysis {
                latin: Some(Algorithm::Spanish),
                ..base
            },
            "de-nofold" => Analysis {
                latin: Some(Algorithm::German),
                fold_latin: false,
                ..base
            },
            "fr-nofold" => Analysis {
                latin: Some(Algorithm::French),
                fold_latin: false,
                ..base
            },
            "es-nofold" => Analysis {
                latin: Some(Algorithm::Spanish),
                fold_latin: false,
                ..base
            },
            other => panic!("no analysis {other}"),
        }
    }
}

/// A token: the word as written, normalized, and its stem when that differs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub surface: String,
    pub stem: Option<String>,
}

pub struct Analyzer {
    analysis: Analysis,
    russian: Stemmer,
    latin: Option<Stemmer>,
    #[cfg(feature = "icu")]
    icu: icu_segmenter::WordSegmenterBorrowed<'static>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Script {
    Latin,
    Cyrillic,
    /// Written without spaces between words: Han, kana, Thai, Lao, Khmer, Myanmar.
    Unspaced,
    Other,
}

fn script(c: char) -> Script {
    match c as u32 {
        0x0041..=0x024F | 0x1E00..=0x1EFF => Script::Latin,
        0x0400..=0x052F | 0x1C80..=0x1C8F | 0x2DE0..=0x2DFF | 0xA640..=0xA69F => Script::Cyrillic,
        0x0E00..=0x0EFF // Thai, Lao
        | 0x1000..=0x109F // Myanmar
        | 0x1780..=0x17FF // Khmer
        | 0x3040..=0x30FF // kana
        | 0x31F0..=0x31FF
        | 0x3400..=0x4DBF // Han
        | 0x4E00..=0x9FFF
        | 0xF900..=0xFAFF
        | 0x20000..=0x3134F => Script::Unspaced,
        _ => Script::Other,
    }
}

impl Analyzer {
    pub fn new(analysis: Analysis) -> Analyzer {
        Analyzer {
            analysis,
            russian: Stemmer::create(Algorithm::Russian),
            latin: analysis.latin.map(Stemmer::create),
            #[cfg(feature = "icu")]
            icu: icu_segmenter::WordSegmenter::new_auto(Default::default()),
        }
    }

    /// The tokens of `text`, in order.
    pub fn tokens(&self, text: &str) -> Vec<Token> {
        let normal = self.normalize(text);
        let mut tokens = Vec::new();
        for word in self.words(&normal) {
            let stem = self.stem(&word);
            tokens.push(Token {
                stem: stem.filter(|stem| *stem != word),
                surface: word,
            });
        }
        tokens
    }

    /// NFKC, lower case, the marks a Latin, Cyrillic or Greek letter carries
    /// apart taken off (Russian Wikipedia's stress marks), ё as е, and Latin
    /// accents folded when the analysis says so.
    fn normalize(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut base = Script::Other;
        for c in text.nfkc().flat_map(char::to_lowercase) {
            if is_combining_mark(c) {
                if matches!(base, Script::Latin | Script::Cyrillic)
                    || (0x0370..=0x03FF).contains(&(base_char(&out) as u32))
                {
                    continue;
                }
                out.push(c);
                continue;
            }
            base = script(c);
            match c {
                'ё' => out.push('е'),
                c if self.analysis.fold_latin && base == Script::Latin && !c.is_ascii() => {
                    out.extend(std::iter::once(c).nfd().filter(|c| !is_combining_mark(*c)));
                }
                c => out.push(c),
            }
        }
        out
    }

    /// Words of letters, marks and digits; a run of an unspaced script as the
    /// analysis segments it.
    ///
    /// Unicode's word rules split Han and Thai a character at a time, so a
    /// run of an unspaced script is cut out first and segmented whole.
    fn words(&self, text: &str) -> Vec<String> {
        let mut words = Vec::new();
        let (mut spaced_from, mut run_from) = (0, None);
        for (at, c) in text.char_indices() {
            let unspaced =
                script(c) == Script::Unspaced && (c.is_alphanumeric() || is_combining_mark(c));
            match (unspaced, run_from) {
                (true, None) => {
                    spaced(&text[spaced_from..at], &mut words);
                    run_from = Some(at);
                }
                (false, Some(from)) => {
                    self.unspaced(&text[from..at], &mut words);
                    (spaced_from, run_from) = (at, None);
                }
                _ => {}
            }
        }
        match run_from {
            Some(from) => self.unspaced(&text[from..], &mut words),
            None => spaced(&text[spaced_from..], &mut words),
        }
        words
    }

    fn unspaced(&self, run: &str, words: &mut Vec<String>) {
        match self.analysis.segment {
            Segment::Bigrams => bigrams(run, words),
            #[cfg(feature = "icu")]
            Segment::Icu => {
                let mut at = 0;
                for (end, kind) in self.icu.segment_str(run).iter_with_word_type() {
                    if end > at && kind.is_word_like() {
                        words.push(run[at..end].to_owned());
                    }
                    at = end;
                }
            }
            Segment::Words => words.push(run.to_owned()),
        }
    }

    fn stem(&self, word: &str) -> Option<String> {
        let first = word.chars().next()?;
        let stemmer = match script(first) {
            Script::Cyrillic if self.analysis.stem_cyrillic => &self.russian,
            Script::Latin => self.latin.as_ref()?,
            _ => return None,
        };
        Some(stemmer.stem(word).into_owned())
    }
}

/// Words by Unicode's rules, kept to their letters, marks and digits:
/// `can't` → `cant`, so that no token holds what the index splits on.
fn spaced(text: &str, words: &mut Vec<String>) {
    for word in text.unicode_words() {
        let word: String = word
            .chars()
            .filter(|c| c.is_alphanumeric() || is_combining_mark(*c))
            .collect();
        if !word.is_empty() {
            words.push(word);
        }
    }
}

/// The last character pushed, for a mark that follows it.
fn base_char(out: &str) -> char {
    out.chars().next_back().unwrap_or(' ')
}

/// Overlapping pairs of a run's graphemes, so that a Thai vowel stays with
/// its consonant: `世界和平` → `世界 界和 和平`; one grapheme alone.
fn bigrams(run: &str, words: &mut Vec<String>) {
    let graphemes: Vec<&str> = run.graphemes(true).collect();
    if graphemes.len() == 1 {
        words.push(run.to_owned());
        return;
    }
    for pair in graphemes.windows(2) {
        words.push(pair.concat());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn surfaces(analysis: &str, text: &str) -> Vec<String> {
        Analyzer::new(Analysis::named(analysis))
            .tokens(text)
            .into_iter()
            .map(|token| token.surface)
            .collect()
    }

    #[test]
    fn stress_marks_and_yo_go_and_short_i_stays() {
        assert_eq!(
            surfaces("auto", "Кари́бский ЁЖ йод"),
            ["карибский", "еж", "йод"]
        );
    }

    #[test]
    fn unspaced_scripts_become_bigrams_beside_spaced_words() {
        assert_eq!(
            surfaces("auto", "café 世界和平 abc"),
            ["cafe", "世界", "界和", "和平", "abc"]
        );
        assert_eq!(
            surfaces("auto", "東京タワー"),
            ["東京", "京タ", "タワ", "ワー"]
        );
        assert_eq!(surfaces("auto", "สวัสดี"), ["สวั", "วัส", "สดี"]);
    }

    #[test]
    fn stems_are_kept_only_where_they_differ() {
        let tokens = Analyzer::new(Analysis::named("auto")).tokens("кошки running cat");
        let stems: Vec<_> = tokens.iter().map(|token| token.stem.as_deref()).collect();
        assert_eq!(stems, [Some("кошк"), Some("run"), None]);
    }

    #[test]
    fn latin_accents_fold_unless_told_not_to() {
        assert_eq!(surfaces("de", "schön"), ["schon"]);
        assert_eq!(surfaces("de-nofold", "schön"), ["schön"]);
    }
}
