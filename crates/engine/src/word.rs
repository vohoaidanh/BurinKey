//! The word being typed and the edits a key can make to it.
//!
//! A word is a list of letters plus one tone for the whole word; where the
//! tone mark shows is worked out on every render, so it moves by itself as
//! the word grows ("tóa" → "toán").
//!
//! Every mark key works the same way: build the edited word, then keep it
//! only if [`spelling::could_become`] says it can still be Vietnamese.
//! Otherwise the key is typed as a plain letter. That one rule gives free
//! mark placement ("chungws" → "chứng"), spell checking ("class" stays
//! "class") and English words passing through untouched.

use unicode_normalization::UnicodeNormalization;

use crate::spelling::{self, Tone, TonePlacement};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Letter {
    /// Lowercase, with any shape mark (ă â ê ô ơ ư đ); non-letters as typed.
    ch: char,
    upper: bool,
    /// Typed by a lone Telex `w` (so a second `w` takes it back out).
    from_w: bool,
}

impl Letter {
    fn typed(key: char) -> Self {
        Self {
            ch: key.to_lowercase().next().unwrap_or(key),
            upper: key.is_uppercase(),
            from_w: false,
        }
    }

    fn render(self) -> char {
        if self.upper {
            self.ch.to_uppercase().next().unwrap_or(self.ch)
        } else {
            self.ch
        }
    }
}

/// Which marks a hook key may add: Telex `w` adds either, VNI splits them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hook {
    /// ă, ơ, ư
    Any,
    /// ơ, ư
    Horn,
    /// ă
    Breve,
}

impl Hook {
    /// The hooked form of `c`, if this kind of hook applies to it.
    fn apply(self, c: char) -> Option<char> {
        match (self, c) {
            (Hook::Any | Hook::Breve, 'a') => Some('ă'),
            (Hook::Any | Hook::Horn, 'o') => Some('ơ'),
            (Hook::Any | Hook::Horn, 'u') => Some('ư'),
            _ => None,
        }
    }

    fn is_hooked(self, c: char) -> bool {
        match self {
            Hook::Any => matches!(c, 'ă' | 'ơ' | 'ư'),
            Hook::Horn => matches!(c, 'ơ' | 'ư'),
            Hook::Breve => c == 'ă',
        }
    }
}

fn without_marks(c: char) -> char {
    match c {
        'ă' | 'â' => 'a',
        'ê' => 'e',
        'ô' | 'ơ' => 'o',
        'ư' => 'u',
        'đ' => 'd',
        other => other,
    }
}

fn with_circumflex(c: char) -> Option<char> {
    match c {
        'a' | 'ă' => Some('â'),
        'e' => Some('ê'),
        'o' | 'ơ' => Some('ô'),
        _ => None,
    }
}

pub struct Word {
    letters: Vec<Letter>,
    tone: Tone,
    placement: TonePlacement,
    /// Every key typed into this word, for restoring a non-Vietnamese word.
    keys: String,
    /// Backspace breaks the link between keys and letters; no restore then.
    keys_match: bool,
}

impl Word {
    pub fn new(placement: TonePlacement) -> Self {
        Self {
            letters: Vec::new(),
            tone: Tone::None,
            placement,
            keys: String::new(),
            keys_match: true,
        }
    }

    pub fn placement(&self) -> TonePlacement {
        self.placement
    }

    pub fn set_placement(&mut self, placement: TonePlacement) {
        self.placement = placement;
    }

    pub fn clear(&mut self) {
        *self = Self::new(self.placement);
    }

    pub fn record_key(&mut self, key: char) {
        self.keys.push(key);
    }

    pub fn render(&self) -> String {
        let carrier = self.tone_carrier();
        let mut out = String::new();
        for (i, letter) in self.letters.iter().enumerate() {
            out.push(letter.render());
            if Some(i) == carrier {
                out.extend(self.tone.combining_mark());
            }
        }
        out.nfc().collect()
    }

    /// The raw keys, if the word ends up neither Vietnamese nor plain:
    /// "ưindows" gives back "windows".
    pub fn restore_text(&self) -> Option<String> {
        let marked =
            self.tone != Tone::None || self.letters.iter().any(|l| "ăâêôơưđ".contains(l.ch));
        let vietnamese = spelling::is_syllable(&chars(&self.letters), self.tone);
        (self.keys_match && marked && !vietnamese).then(|| self.keys.clone())
    }

    /// Deletes the last letter. Its tone goes with it if it carried the tone.
    pub fn pop(&mut self) -> bool {
        if self.letters.is_empty() {
            return false;
        }
        if self.tone_carrier() == Some(self.letters.len() - 1) {
            self.tone = Tone::None;
        }
        self.letters.pop();
        self.keys_match = false;
        true
    }

    // -- Edits -------------------------------------------------------------

    /// Types `key` as it is.
    pub fn type_letter(&mut self, key: char) {
        self.letters.push(Letter::typed(key));
        // Keep the word tidy when it can be ("trưon" → "trươn").
        let mut tidy = self.letters.clone();
        tidy_horns(&mut tidy);
        if spelling::could_become(&chars(&tidy), self.tone) {
            self.letters = tidy;
        }
    }

    pub fn set_tone(&mut self, tone: Tone, key: char) {
        if self.tone == tone {
            // Same tone again: take it off and type the key instead.
            self.tone = Tone::None;
            return self.type_letter(key);
        }
        let has_vowel = self.letters.iter().any(|l| spelling::is_vowel(l.ch));
        let applied = has_vowel
            && self.try_edit(|letters, t| {
                *t = tone;
                open_uo(letters);
            });
        if !applied {
            self.type_letter(key);
        }
    }

    pub fn remove_tone(&mut self, key: char) {
        if self.tone == Tone::None {
            self.type_letter(key);
        } else {
            self.tone = Tone::None;
        }
    }

    /// â ê ô on the last matching vowel (`family` 'a', 'e' or 'o'; `None`
    /// for any). A second press takes it off and types the key.
    pub fn add_circumflex(&mut self, family: Option<char>, key: char) {
        let target = self.nucleus().and_then(|range| {
            range.rev().find(|&i| {
                let base = without_marks(self.letters[i].ch);
                family.map_or(matches!(base, 'a' | 'e' | 'o'), |f| base == f)
            })
        });
        let Some(i) = target else {
            return self.type_letter(key);
        };
        if matches!(self.letters[i].ch, 'â' | 'ê' | 'ô') {
            self.letters[i].ch = without_marks(self.letters[i].ch);
            return self.type_letter(key);
        }
        let applied = self.try_edit(|letters, _| {
            if let Some(c) = with_circumflex(letters[i].ch) {
                letters[i].ch = c;
            }
        });
        if !applied {
            self.type_letter(key);
        }
    }

    /// Adds a breve/horn, trying in turn: the "ươ" pair, undoing an
    /// existing hook (second press), then each vowel from the right.
    /// Returns `false` if nothing could take it, so the caller decides what
    /// the key means on its own.
    pub fn add_hook(&mut self, hook: Hook, key: char) -> bool {
        let Some(nucleus) = self.nucleus() else {
            return false;
        };

        if hook != Hook::Breve {
            let pair = nucleus.clone().find(|&i| {
                i + 1 < nucleus.end
                    && matches!(self.letters[i].ch, 'u' | 'ư')
                    && matches!(self.letters[i + 1].ch, 'o' | 'ơ')
                    && (self.letters[i].ch, self.letters[i + 1].ch) != ('ư', 'ơ')
            });
            if let Some(i) = pair {
                if self.try_edit(|letters, _| {
                    letters[i].ch = 'ư';
                    letters[i + 1].ch = 'ơ';
                }) {
                    return true;
                }
            }
        }

        let hooked: Vec<usize> = nucleus
            .clone()
            .filter(|&i| hook.is_hooked(self.letters[i].ch))
            .collect();
        if !hooked.is_empty() {
            for &i in hooked.iter().rev() {
                if self.letters[i].from_w {
                    self.letters.remove(i);
                } else {
                    self.letters[i].ch = without_marks(self.letters[i].ch);
                }
            }
            self.type_letter(key);
            return true;
        }

        nucleus.rev().any(|i| {
            hook.apply(self.letters[i].ch)
                .is_some_and(|c| self.try_edit(|letters, _| letters[i].ch = c))
        })
    }

    /// Adds a vowel that has its own key (Telex lone `w` → ư, `[` → ơ) if
    /// the word allows it; otherwise types `key`.
    pub fn insert_vowel(&mut self, vowel: char, upper: bool, from_w: bool, key: char) {
        let letter = Letter {
            ch: vowel,
            upper,
            from_w,
        };
        if !self.try_edit(|letters, _| letters.push(letter)) {
            self.type_letter(key);
        }
    }

    /// d → đ on the first letter; a second press takes it off.
    pub fn stroke_d(&mut self, key: char) {
        match self.letters.first().map(|l| l.ch) {
            Some('d') if self.try_edit(|letters, _| letters[0].ch = 'đ') => {}
            Some('đ') => {
                self.letters[0].ch = 'd';
                self.type_letter(key);
            }
            _ => self.type_letter(key),
        }
    }

    // -- Helpers -----------------------------------------------------------

    /// Applies `edit` to a copy and keeps it if the word can still be
    /// Vietnamese.
    fn try_edit(&mut self, edit: impl FnOnce(&mut Vec<Letter>, &mut Tone)) -> bool {
        let mut letters = self.letters.clone();
        let mut tone = self.tone;
        edit(&mut letters, &mut tone);
        tidy_horns(&mut letters);
        if !spelling::could_become(&chars(&letters), tone) {
            return false;
        }
        self.letters = letters;
        self.tone = tone;
        true
    }

    fn nucleus(&self) -> Option<std::ops::Range<usize>> {
        spelling::parts(&chars(&self.letters)).map(|p| p.nucleus)
    }

    fn tone_carrier(&self) -> Option<usize> {
        if self.tone == Tone::None {
            return None;
        }
        spelling::tone_carrier(&chars(&self.letters), self.placement)
    }
}

fn chars(letters: &[Letter]) -> Vec<char> {
    letters.iter().map(|l| l.ch).collect()
}

/// u/ư followed by o/ô/ơ only has three spellings — uô, ươ, and uơ at the
/// very end ("thuở") — so fix the others once the next letter shows which
/// one is meant: ưô → uô; ưo or uơ with something after → ươ.
fn tidy_horns(letters: &mut [Letter]) {
    let Some(p) = spelling::parts(&chars(letters)) else {
        return;
    };
    for i in p.nucleus.clone() {
        if i + 1 >= letters.len() {
            break;
        }
        let followed = i + 2 < letters.len();
        match (letters[i].ch, letters[i + 1].ch) {
            ('ư', 'ô') => letters[i].ch = 'u',
            ('ư', 'o') if followed => letters[i + 1].ch = 'ơ',
            ('u', 'ơ') if followed => letters[i].ch = 'ư',
            _ => {}
        }
    }
}

/// "ươ" can't end a syllable; when a tone lands on a word ending that way
/// it must be the open "uơ" ("thươ" + hỏi → "thuở").
fn open_uo(letters: &mut [Letter]) {
    let n = letters.len();
    if n >= 2 && letters[n - 2].ch == 'ư' && letters[n - 1].ch == 'ơ' {
        letters[n - 2].ch = 'u';
    }
}
