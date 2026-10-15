//! VNI: digits 1–5 add a tone (sắc, huyền, hỏi, ngã, nặng) and 0 removes
//! it; 6 adds a circumflex, 7 a horn, 8 a breve, 9 turns d into đ.

use crate::spelling::Tone;
use crate::word::{Hook, Word};

pub fn handle_key(word: &mut Word, key: char) {
    match key {
        '0' => word.remove_tone(key),
        '1' => word.set_tone(Tone::Acute, key),
        '2' => word.set_tone(Tone::Grave, key),
        '3' => word.set_tone(Tone::Hook, key),
        '4' => word.set_tone(Tone::Tilde, key),
        '5' => word.set_tone(Tone::Dot, key),
        '6' => word.add_circumflex(None, key),
        '7' => hook_or_type(word, Hook::Horn, key),
        '8' => hook_or_type(word, Hook::Breve, key),
        '9' => word.stroke_d(key),
        _ => word.type_letter(key),
    }
}

fn hook_or_type(word: &mut Word, hook: Hook, key: char) {
    if !word.add_hook(hook, key) {
        word.type_letter(key);
    }
}
