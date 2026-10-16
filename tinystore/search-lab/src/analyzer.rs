#[derive(Clone, Debug)]
pub struct Token {
    pub text: String,
    pub start: usize,
    pub end: usize,
    pub position: usize,
}
#[derive(Clone)]
pub struct Analyzer {
    pub mode: String,
    #[cfg(feature = "icu")]
    segmenter: icu_segmenter::WordSegmenterBorrowed<'static>,
    #[cfg(feature = "stems")]
    stems: std::sync::Arc<Vec<(&'static str, rust_stemmers::Stemmer)>>,
    #[cfg(feature = "jieba")]
    jieba: Option<std::sync::Arc<jieba_rs::Jieba>>,
}
impl Analyzer {
    pub fn new(mode: &str) -> Self {
        assert!(matches!(
            mode,
            "unicode" | "icu" | "auto" | "multi" | "detect" | "hybrid" | "jieba"
        ));
        #[cfg(not(feature = "icu"))]
        assert_eq!(mode, "unicode", "ICU feature required");
        #[cfg(not(feature = "stems"))]
        assert!(matches!(mode, "unicode" | "icu"), "stems feature required");
        Self {
            mode: mode.into(),
            #[cfg(feature = "icu")]
            segmenter: icu_segmenter::WordSegmenter::new_auto(Default::default()),
            #[cfg(feature = "stems")]
            stems: {
                use rust_stemmers::{Algorithm::*, Stemmer};
                [
                    ("ru", Russian),
                    ("en", English),
                    ("de", German),
                    ("fr", French),
                    ("es", Spanish),
                ]
                .into_iter()
                .map(|(n, a)| (n, Stemmer::create(a)))
                .collect::<Vec<_>>()
                .into()
            },
            #[cfg(feature = "jieba")]
            jieba: if mode == "jieba" {
                Some(std::sync::Arc::new(jieba_rs::Jieba::new()))
            } else {
                None
            },
        }
    }
    pub fn tokens(&self, text: &str) -> Vec<Token> {
        self.tokens_role(text, false)
    }
    pub fn query_tokens(&self, text: &str) -> Vec<Token> {
        self.tokens_role(text, true)
    }
    pub fn tokens_role(&self, text: &str, is_query: bool) -> Vec<Token> {
        #[cfg(not(feature = "icu"))]
        {
            let _ = text;
            unreachable!("native unicode61 is implemented by SQLite")
        }
        #[cfg(feature = "icu")]
        {
            use unicode_normalization::UnicodeNormalization;
            let mut spans = vec![];
            #[cfg(feature = "jieba")]
            if let Some(j) = &self.jieba {
                for t in j.cut(text, true) {
                    let start = t.word.as_ptr() as usize - text.as_ptr() as usize;
                    spans.push((start, start + t.word.len()));
                }
            }
            if spans.is_empty() {
                let mut a = 0;
                for b in self.segmenter.segment_str(text).skip(1) {
                    spans.push((a, b));
                    a = b;
                }
            }
            #[cfg(feature = "detect")]
            let detected = if self.mode == "detect" || self.mode == "hybrid" && !is_query {
                whatlang::detect(text)
                    .filter(|i| i.is_reliable())
                    .map(|i| i.lang().code())
            } else {
                None
            };
            #[cfg(not(feature = "detect"))]
            let detected: Option<&str> = None;
            let mut out = vec![];
            let mut position = 0;
            for (start, end) in spans {
                let original = &text[start..end];
                if !original.chars().any(char::is_alphanumeric) {
                    continue;
                }
                let word: String = original
                    .nfkc()
                    .flat_map(char::to_lowercase)
                    .map(|c| if c == 'ё' { 'е' } else { c })
                    .collect();
                let mut terms = vec![word.clone()];
                #[cfg(feature = "stems")]
                if self.mode != "icu" && self.mode != "unicode" {
                    let cyr = word.chars().any(|c| ('\u{0400}'..='\u{052f}').contains(&c));
                    let latin = word
                        .chars()
                        .any(|c| c.is_ascii_alphabetic() || ('\u{00c0}'..='\u{024f}').contains(&c));
                    let lang = match detected {
                        Some("deu") => "de",
                        Some("fra") => "fr",
                        Some("spa") => "es",
                        _ => "en",
                    };
                    for (n, s) in self.stems.iter() {
                        if (cyr && *n == "ru")
                            || (latin
                                && (*n
                                    == if matches!(self.mode.as_str(), "detect" | "hybrid") {
                                        lang
                                    } else {
                                        "en"
                                    }
                                    || (self.mode == "multi" || self.mode == "hybrid" && is_query)
                                        && *n != "ru"))
                        {
                            terms.push(format!("s_{n}_{}", s.stem(&word)));
                        }
                    }
                }
                terms.sort();
                terms.dedup();
                for term in terms {
                    out.push(Token {
                        text: term,
                        start,
                        end,
                        position,
                    });
                }
                position += 1;
            }
            let _ = detected;
            out
        }
    }
}
