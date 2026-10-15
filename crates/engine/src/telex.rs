//! Telex: s f r x j add a tone and z removes it; aa ee oo give â ê ô;
//! w adds a breve/horn; dd gives đ. Full Telex also types ư for a lone `w`
//! and ơ/ư for `[`/`]`. Simple Telex leaves those keys alone, so "pwm" stays
//! "pwm".

use crate::spelling::Tone;
use crate::word::{Hook, Word};

pub fn handle_key(word: &mut Word, key: char, simple: bool) {
    let upper = key.is_uppercase();
    match key.to_ascii_lowercase() {
        's' => word.set_tone(Tone::Acute, key),
        'f' => word.set_tone(Tone::Grave, key),
        'r' => word.set_tone(Tone::Hook, key),
        'x' => word.set_tone(Tone::Tilde, key),
        'j' => word.set_tone(Tone::Dot, key),
        'z' => word.remove_tone(key),
        family @ ('a' | 'e' | 'o') => word.add_circumflex(Some(family), key),
        'd' => word.stroke_d(key),
        'w' => {
            if !word.add_hook(Hook::Any, key) {
                if simple {
                    word.type_letter(key);
                } else {
                    word.insert_vowel('ư', upper, true, key);
                }
            }
        }
        _ if !simple && is_vowel_key(key) => {
            let (vowel, upper) = match key {
                '[' => ('ơ', false),
                ']' => ('ư', false),
                '{' => ('ơ', true),
                _ => ('ư', true),
            };
            word.insert_vowel(vowel, upper, false, key);
        }
        _ => word.type_letter(key),
    }
}

/// Bracket keys that type ơ/ư in full Telex (so they don't end the word).
pub fn is_vowel_key(key: char) -> bool {
    matches!(key, '[' | ']' | '{' | '}')
}
