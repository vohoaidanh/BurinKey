//! What a written Vietnamese syllable may look like (quốc ngữ spelling).
//!
//! A syllable is `onset + nucleus + coda`: an optional initial consonant
//! group, a vowel cluster, and an optional final consonant. Each vowel
//! cluster only accepts certain codas, and a few onsets depend on the vowel
//! that follows (k/gh/ngh before e ê i y, c/g/ng elsewhere).
//!
//! The engine asks two questions:
//! - [`could_become`]: can the letters typed so far still turn into a real
//!   syllable once more marks (and letters) are added? Used to accept or
//!   reject each mark while typing.
//! - [`is_syllable`]: is this exactly a real syllable? Used at the end of a
//!   word to decide whether to give back the raw keys.
//!
//! [`tone_carrier`] decides which vowel shows the tone mark.
//!
//! Letters are lowercase chars with their shape marks (ă â ê ô ơ ư đ); the
//! tone is kept separately as a [`Tone`].

use std::ops::Range;

/// Where the tone goes on "oa", "oe", "uy" with nothing after them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TonePlacement {
    /// "hòa", "thúy" — the standard style, used by default.
    Modern,
    /// "hoà", "thuý".
    Classic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tone {
    #[default]
    None,
    /// sắc
    Acute,
    /// huyền
    Grave,
    /// hỏi
    Hook,
    /// ngã
    Tilde,
    /// nặng
    Dot,
}

impl Tone {
    /// The Unicode combining mark that draws this tone.
    pub fn combining_mark(self) -> Option<char> {
        match self {
            Tone::None => None,
            Tone::Acute => Some('\u{301}'),
            Tone::Grave => Some('\u{300}'),
            Tone::Hook => Some('\u{309}'),
            Tone::Tilde => Some('\u{303}'),
            Tone::Dot => Some('\u{323}'),
        }
    }

    /// Syllables ending in c, ch, p, t only take sắc or nặng (or no tone
    /// yet, while still being typed).
    fn fits_stop_coda(self) -> bool {
        matches!(self, Tone::None | Tone::Acute | Tone::Dot)
    }
}

// The two tables below hold UniKey's syllable data (its consonant list
// `CSeqList`, and the complete vowel sequences of `VSeqList` with their
// final consonants from `VCPairList`), rewritten in this file's format.
// UniKey Vietnamese Input Method, Copyright (C) 2000-2005 Pham Kim Long,
// http://unikey.org, GNU LGPL v2 or later (used here under GPL v3+).
// Only the data is taken; the code that uses it is this project's own.

const ONSETS: &[&str] = &[
    "b", "c", "ch", "d", "dz", "đ", "g", "gh", "h", "k", "kh", "l", "m", "n", "ng", "ngh", "nh",
    "p", "ph", "q", "r", "s", "t", "th", "tr", "v", "x",
];

/// Vowel clusters and what may follow each: `-` means the syllable may end
/// right after the cluster; the other entries are the allowed final
/// consonants.
const NUCLEI: &[(&str, &str)] = &[
    // One vowel.
    ("a", "- c ch m n ng nh p t"),
    ("ă", "- c m n ng p t"),
    ("â", "- c m n ng p t"),
    ("e", "- c ch m n ng nh p t"),
    ("ê", "- c ch m n nh p t"),
    ("i", "- c ch m n nh p t"),
    ("o", "- c m n ng p t"),
    ("ô", "- c m n ng p t"),
    ("ơ", "- m n p t"),
    ("u", "- c m n ng p t"),
    ("ư", "- c m n ng t"),
    ("y", "- t"),
    // Two vowels.
    ("ai", "-"),
    ("ao", "-"),
    ("au", "-"),
    ("ay", "-"),
    ("âu", "-"),
    ("ây", "-"),
    ("eo", "-"),
    ("êu", "-"),
    ("ia", "-"),
    ("iê", "- c m n ng p t"),
    ("iu", "-"),
    ("oa", "- c ch m n ng nh p t"),
    ("oă", "- c m n ng t"),
    ("oe", "- n t"),
    ("oi", "-"),
    ("ôi", "-"),
    ("ơi", "-"),
    ("ua", "- n ng t"),
    ("uâ", "- n ng t"),
    ("uê", "- c ch n nh"),
    ("ui", "-"),
    ("uô", "- c m n ng t"),
    ("uơ", "-"),
    ("uy", "- c ch n nh p t"),
    ("ưa", "-"),
    ("ưi", "-"),
    ("ươ", "- c m n ng p t"),
    ("ưu", "-"),
    ("yê", "- m n ng t"),
    // Three vowels.
    ("iêu", "-"),
    ("oai", "-"),
    ("oay", "-"),
    ("oeo", "-"),
    ("uây", "-"),
    ("uôi", "-"),
    ("uya", "-"),
    ("uyê", "- n t"),
    ("uyu", "-"),
    ("ươi", "-"),
    ("ươu", "-"),
    ("yêu", "-"),
    // Added by this project (not in UniKey's data): "xoong", "ngoao".
    ("oo", "c ng"),
    ("oao", "-"),
];

pub fn is_vowel(c: char) -> bool {
    "aăâeêioôơuưy".contains(c)
}

/// Vowels that carry a shape mark (breve, circumflex or horn).
fn has_shape_mark(c: char) -> bool {
    "ăâêôơư".contains(c)
}

/// What a plain vowel could still become once a mark is typed.
fn mark_variants(c: char) -> &'static [char] {
    match c {
        'a' => &['a', 'ă', 'â'],
        'e' => &['e', 'ê'],
        'o' => &['o', 'ô', 'ơ'],
        'u' => &['u', 'ư'],
        'ă' => &['ă'],
        'â' => &['â'],
        'ê' => &['ê'],
        'i' => &['i'],
        'ô' => &['ô'],
        'ơ' => &['ơ'],
        'ư' => &['ư'],
        'y' => &['y'],
        _ => &[],
    }
}

/// The parts of a word, by letter index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parts {
    /// Leading consonants, before any vowel.
    consonants: usize,
    /// The vowel cluster. When the onset is "qu" or "gi", its u/i is not
    /// part of the nucleus: "quá" has nucleus "a", "già" has nucleus "a".
    pub nucleus: Range<usize>,
    /// Number of letters after the vowels.
    pub coda_len: usize,
}

/// Splits `word` into consonants + vowels + consonants. `None` if a vowel
/// shows up after the final consonants ("facebook").
pub fn parts(word: &[char]) -> Option<Parts> {
    let consonants = word.iter().take_while(|c| !is_vowel(**c)).count();
    let vowels_end = consonants
        + word[consonants..]
            .iter()
            .take_while(|c| is_vowel(**c))
            .count();
    if word[vowels_end..].iter().any(|c| is_vowel(*c)) {
        return None;
    }
    let vowels = &word[consonants..vowels_end];
    let onset = &word[..consonants];
    let glued = vowels.len() >= 2
        && ((onset == ['q'] && vowels[0] == 'u') || (onset == ['g'] && vowels[0] == 'i'));
    Some(Parts {
        consonants,
        nucleus: consonants + usize::from(glued)..vowels_end,
        coda_len: word.len() - vowels_end,
    })
}

/// Can `word` (with `tone`) still become a real syllable if the user adds
/// marks or more letters?
pub fn could_become(word: &[char], tone: Tone) -> bool {
    check(word, tone, false)
}

/// Is `word` exactly a real syllable as it stands?
pub fn is_syllable(word: &[char], tone: Tone) -> bool {
    check(word, tone, true)
}

fn check(word: &[char], tone: Tone, complete: bool) -> bool {
    let Some(p) = parts(word) else {
        return false;
    };
    let onset: String = word[..p.consonants].iter().collect();
    let vowels = &word[p.consonants..p.nucleus.end];
    let coda: String = word[p.nucleus.end..].iter().collect();

    if vowels.is_empty() {
        // Only consonants so far ("ngh", "đ"): fine while typing.
        return !complete && is_onset_prefix(&onset);
    }
    if !onset.is_empty() && !ONSETS.contains(&onset.as_str()) {
        return false;
    }
    if !onset_fits_vowel(&onset, vowels[0]) {
        return false;
    }
    if onset == "q" && vowels.len() == 1 {
        // "qu" waiting for its vowel.
        return !complete && coda.is_empty();
    }
    if coda.starts_with(['c', 'p', 't']) && !tone.fits_stop_coda() {
        return false;
    }

    // For "qu"/"gi" the rhyme can be read with or without the glued vowel:
    // "quanh" = qu + anh, "quyết" = q + uyêt, "gieo" = gi + eo,
    // "giếng" = g + iêng.
    let glued = p.nucleus.start > p.consonants;
    let mut readings = vec![vowels];
    if glued {
        readings.push(&vowels[1..]);
    }
    readings.into_iter().any(|cluster| {
        // "yê" opens a syllable only with no onset ("yên"); after a
        // consonant it is spelled "iê".
        let y_misplaced = cluster.starts_with(&['y', 'ê']) && !onset.is_empty() && !glued;
        !y_misplaced && rhyme_fits(cluster, &coda, complete)
    })
}

fn is_onset_prefix(onset: &str) -> bool {
    ONSETS.iter().any(|o| o.starts_with(onset))
}

/// k, gh, ngh only go before e ê i y; c, ng never do, and g only before i
/// (as "gi").
fn onset_fits_vowel(onset: &str, vowel: char) -> bool {
    let front = matches!(vowel, 'e' | 'ê' | 'i' | 'y');
    match onset {
        "k" | "gh" | "ngh" => front,
        "c" | "ng" => !front,
        "g" => !matches!(vowel, 'e' | 'ê' | 'y'),
        "q" => vowel == 'u',
        _ => true,
    }
}

/// Does `cluster` + `coda` make a rhyme? Without `complete`, plain vowels
/// may still gain marks, the cluster may still grow, and the coda may still
/// grow ("n" → "ng").
fn rhyme_fits(cluster: &[char], coda: &str, complete: bool) -> bool {
    let spellings: Vec<String> = if complete {
        vec![cluster.iter().collect()]
    } else {
        with_marks(cluster)
    };
    spellings.iter().any(|spelling| {
        NUCLEI.iter().any(|(nucleus, codas)| {
            let mut codas = codas.split_whitespace();
            match (coda.is_empty(), complete) {
                (true, true) => nucleus == spelling && codas.any(|c| c == "-"),
                (true, false) => nucleus.starts_with(spelling.as_str()),
                (false, true) => nucleus == spelling && codas.any(|c| c == coda),
                (false, false) => {
                    nucleus == spelling && codas.any(|c| c != "-" && c.starts_with(coda))
                }
            }
        })
    })
}

/// Every spelling `cluster` could reach by adding shape marks.
fn with_marks(cluster: &[char]) -> Vec<String> {
    cluster.iter().fold(vec![String::new()], |prefixes, &c| {
        prefixes
            .iter()
            .flat_map(|p| mark_variants(c).iter().map(move |v| format!("{p}{v}")))
            .collect()
    })
}

/// Index of the letter that shows the tone mark, or `None` with no vowel.
pub fn tone_carrier(word: &[char], placement: TonePlacement) -> Option<usize> {
    match parts(word) {
        Some(p) if !p.nucleus.is_empty() => {
            let nucleus = &word[p.nucleus.clone()];
            Some(p.nucleus.start + position_in_nucleus(nucleus, p.coda_len > 0, placement))
        }
        // Not a syllable: keep the mark on the first vowel.
        _ => word.iter().position(|c| is_vowel(*c)),
    }
}

/// Quốc ngữ tone placement:
/// 1. a single vowel takes it;
/// 2. otherwise a vowel with a shape mark (the last one: "ươ" → ơ);
/// 3. otherwise, with a final consonant, the last vowel ("toán");
/// 4. otherwise the middle of three vowels ("ngoài");
/// 5. otherwise the first of two ("múa", "tài"), except that the classic
///    style puts it on the second of oa, oe, uy ("hoà", "thuý").
fn position_in_nucleus(nucleus: &[char], has_coda: bool, placement: TonePlacement) -> usize {
    if nucleus.len() == 1 {
        0
    } else if let Some(i) = nucleus.iter().rposition(|c| has_shape_mark(*c)) {
        i
    } else if has_coda {
        nucleus.len() - 1
    } else if nucleus.len() == 3
        || (placement == TonePlacement::Classic
            && matches!(nucleus, ['o', 'a'] | ['o', 'e'] | ['u', 'y']))
    {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn letters(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    fn syllable(s: &str) -> bool {
        is_syllable(&letters(s), Tone::None)
    }

    #[test]
    fn real_syllables() {
        // Letters only: tones are kept apart from the letters.
        for s in [
            "a", "ba", "nghiêng", "ngươi", "quyêt", "giêng", "gi", "gin", "khuya", "thuơ", "quanh",
            "gieo", "yên", "khuyu", "xoong", "dât", "pưm", "oach",
        ] {
            assert!(syllable(s), "{s}");
        }
    }

    #[test]
    fn not_syllables() {
        for s in [
            "ka", "ce", "nge", "gha", "class", "chuac", "uong", "tyên", "windows",
        ] {
            assert!(!syllable(s), "{s}");
        }
    }

    #[test]
    fn unfinished_words_can_still_become_syllables() {
        for s in ["ngh", "đ", "q", "qu", "nguo", "truong", "hoă", "tiê"] {
            assert!(could_become(&letters(s), Tone::None), "{s}");
        }
        assert!(!could_become(&letters("cat"), Tone::Grave));
        assert!(could_become(&letters("cat"), Tone::Acute));
    }
}
