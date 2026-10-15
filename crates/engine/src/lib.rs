//! Platform-independent Vietnamese typing engine.
//!
//! This crate has no I/O and no `unsafe` code: it only transforms a stream of
//! [`KeyEvent`]s into [`Action`]s. The platform layer (see the `platform-win`
//! crate) turns real keystrokes into [`KeyEvent`]s and carries out the
//! resulting [`Action`] on screen.
//!
//! Layout: [`spelling`] knows what a Vietnamese syllable may look like,
//! [`word`] holds the word being typed and the edits a key can make,
//! [`telex`]/[`vni`] map keys to those edits. This module decides where
//! words end, gives non-Vietnamese words back their raw keys, and turns the
//! word's before/after rendering into the backspaces + text an
//! [`Action::Replace`] needs.

mod spelling;
mod telex;
mod vni;
mod word;

pub use spelling::TonePlacement;
use word::Word;

/// A single keystroke as seen by the engine, after keyboard-layout translation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEvent {
    /// A key that types a character (letters, digits, punctuation, space).
    Char(char),
    /// Backspace: delete the previous character.
    Backspace,
    /// Any other key (Enter, Tab, arrows, Escape, focus change, ...). Ends
    /// the word without restoring it.
    WordBoundary,
}

/// What the engine wants the platform layer to do about a processed keystroke.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Let the keystroke through unmodified.
    PassThrough,
    /// Delete `backspaces` characters already on screen, then insert `insert`.
    Replace { backspaces: u8, insert: String },
}

/// Typing mode. Switching mode resets any in-progress word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Vietnamese,
    /// Everything passes through unmodified.
    English,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypingStyle {
    Telex,
    /// Telex where `w` only adds a horn/breve and never types "ư" on its own.
    SimpleTelex,
    Vni,
}

impl TypingStyle {
    fn handle_key(self, word: &mut Word, key: char) {
        match self {
            TypingStyle::Telex => telex::handle_key(word, key, false),
            TypingStyle::SimpleTelex => telex::handle_key(word, key, true),
            TypingStyle::Vni => vni::handle_key(word, key),
        }
    }

    /// Spaces and punctuation end a word; letters and digits don't. In full
    /// Telex the bracket keys type ơ/ư, so they stay inside the word.
    fn is_word_break(self, ch: char) -> bool {
        if self == TypingStyle::Telex && telex::is_vowel_key(ch) {
            return false;
        }
        ch.is_whitespace() || ch.is_ascii_punctuation()
    }
}

/// Keeps track of the word currently being composed and turns keystrokes into
/// actions for the platform layer to apply.
pub struct Engine {
    mode: InputMode,
    style: TypingStyle,
    word: Word,
    auto_restore: bool,
}

impl Engine {
    pub fn new() -> Self {
        Self::with_style(TypingStyle::Telex)
    }

    pub fn with_style(style: TypingStyle) -> Self {
        Self {
            mode: InputMode::Vietnamese,
            style,
            word: Word::new(TonePlacement::Modern),
            auto_restore: true,
        }
    }

    pub fn mode(&self) -> InputMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: InputMode) {
        self.mode = mode;
        self.reset();
    }

    pub fn style(&self) -> TypingStyle {
        self.style
    }

    pub fn set_style(&mut self, style: TypingStyle) {
        self.style = style;
        self.reset();
    }

    pub fn tone_placement(&self) -> TonePlacement {
        self.word.placement()
    }

    pub fn set_tone_placement(&mut self, placement: TonePlacement) {
        self.word.set_placement(placement);
    }

    pub fn auto_restore(&self) -> bool {
        self.auto_restore
    }

    /// "Tự khôi phục phím với từ sai": a word that isn't Vietnamese but
    /// picked up Vietnamese marks gets its original keys back when it ends
    /// ("windows" instead of "ưindows").
    pub fn set_auto_restore(&mut self, on: bool) {
        self.auto_restore = on;
    }

    /// Clears any in-progress word, e.g. on word boundary, mode switch, or focus change.
    pub fn reset(&mut self) {
        self.word.clear();
    }

    /// Processes one keystroke and returns what the platform layer should do.
    pub fn process(&mut self, event: KeyEvent) -> Action {
        if self.mode == InputMode::English {
            if event == KeyEvent::WordBoundary {
                self.reset();
            }
            return Action::PassThrough;
        }

        match event {
            KeyEvent::WordBoundary => {
                self.reset();
                Action::PassThrough
            }
            KeyEvent::Backspace => self.handle_backspace(),
            KeyEvent::Char(ch) => self.handle_char(ch),
        }
    }

    fn handle_char(&mut self, ch: char) -> Action {
        if self.style.is_word_break(ch) {
            let action = self.restore_action(ch);
            self.reset();
            return action;
        }
        if ch.is_control() {
            self.reset();
            return Action::PassThrough;
        }

        let before = self.word.render();
        self.word.record_key(ch);
        self.style.handle_key(&mut self.word, ch);
        let after = self.word.render();
        append_action(&before, &after, ch)
    }

    /// At a word break, swaps a non-Vietnamese word back to its raw keys and
    /// types the break character after it.
    fn restore_action(&self, ch: char) -> Action {
        if !self.auto_restore {
            return Action::PassThrough;
        }
        match self.word.restore_text() {
            Some(raw) => Action::Replace {
                backspaces: self.word.render().chars().count() as u8,
                insert: format!("{raw}{ch}"),
            },
            None => Action::PassThrough,
        }
    }

    fn handle_backspace(&mut self) -> Action {
        let before = self.word.render();
        if !self.word.pop() {
            return Action::PassThrough;
        }
        let after = self.word.render();
        backspace_action(&before, &after)
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

/// Builds the action for a character keystroke by comparing what the word
/// rendered to before and after it. If the result is just the key appended
/// (most keys), passing it through is enough. Otherwise delete back to where
/// the two renderings diverge and type the new tail — which also covers a
/// tone mark moving to an earlier letter ("toas" "tóa" -> "toasn" "toán").
fn append_action(before: &str, after: &str, typed: char) -> Action {
    let naive: String = before.chars().chain(std::iter::once(typed)).collect();
    if naive == after {
        return Action::PassThrough;
    }
    diff_action(before, after)
}

fn backspace_action(before: &str, after: &str) -> Action {
    let before_chars: Vec<char> = before.chars().collect();
    if before_chars.is_empty() {
        return Action::PassThrough;
    }
    let naive: String = before_chars[..before_chars.len() - 1].iter().collect();
    if naive == after {
        return Action::PassThrough;
    }
    diff_action(before, after)
}

/// Deletes back to the longest common prefix between `before` and `after`,
/// then types whatever is left of `after`.
fn diff_action(before: &str, after: &str) -> Action {
    let before_chars: Vec<char> = before.chars().collect();
    let after_chars: Vec<char> = after.chars().collect();
    let common = before_chars
        .iter()
        .zip(after_chars.iter())
        .take_while(|(a, b)| a == b)
        .count();

    Action::Replace {
        backspaces: (before_chars.len() - common) as u8,
        insert: after_chars[common..].iter().collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Types `input` one key at a time and returns what ends up on screen.
    fn type_str(engine: &mut Engine, input: &str) -> String {
        let mut screen = String::new();
        for ch in input.chars() {
            let action = engine.process(KeyEvent::Char(ch));
            apply(&mut screen, action, Some(ch));
        }
        screen
    }

    fn apply(screen: &mut String, action: Action, typed: Option<char>) {
        match action {
            Action::PassThrough => match typed {
                Some(ch) => screen.push(ch),
                None => {
                    screen.pop();
                }
            },
            Action::Replace { backspaces, insert } => {
                for _ in 0..backspaces {
                    screen.pop();
                }
                screen.push_str(&insert);
            }
        }
    }

    fn backspace(engine: &mut Engine, screen: &mut String) {
        let action = engine.process(KeyEvent::Backspace);
        apply(screen, action, None);
    }

    fn typed(style: TypingStyle, input: &str) -> String {
        type_str(&mut Engine::with_style(style), input)
    }

    fn telex(input: &str) -> String {
        typed(TypingStyle::Telex, input)
    }

    fn vni(input: &str) -> String {
        typed(TypingStyle::Vni, input)
    }

    #[test]
    fn telex_marks() {
        assert_eq!(telex("aa"), "â");
        assert_eq!(telex("ee"), "ê");
        assert_eq!(telex("oo"), "ô");
        assert_eq!(telex("aw"), "ă");
        assert_eq!(telex("ow"), "ơ");
        assert_eq!(telex("uw"), "ư");
        assert_eq!(telex("dd"), "đ");
        assert_eq!(telex("Dd"), "Đ");
    }

    #[test]
    fn telex_tones() {
        assert_eq!(telex("as"), "á");
        assert_eq!(telex("af"), "à");
        assert_eq!(telex("ar"), "ả");
        assert_eq!(telex("ax"), "ã");
        assert_eq!(telex("aj"), "ạ");
        assert_eq!(telex("asz"), "a");
        assert_eq!(telex("asf"), "à");
    }

    #[test]
    fn telex_press_again_to_undo() {
        assert_eq!(telex("aaa"), "aa");
        assert_eq!(telex("ass"), "as");
        assert_eq!(telex("ddd"), "dd");
        assert_eq!(telex("uww"), "uw");
    }

    #[test]
    fn telex_lone_w() {
        assert_eq!(telex("w"), "ư");
        assert_eq!(telex("W"), "Ư");
        assert_eq!(telex("tw"), "tư");
        assert_eq!(telex("tws"), "tứ");
        assert_eq!(telex("ww"), "w");
        assert_eq!(telex("[]"), "ơ]");
        assert_eq!(telex("t]"), "tư");
    }

    #[test]
    fn telex_real_words() {
        assert_eq!(telex("vieetj"), "việt");
        assert_eq!(telex("nguyeenx"), "nguyễn");
        assert_eq!(telex("tieengs"), "tiếng");
        assert_eq!(telex("chaof"), "chào");
        assert_eq!(telex("duwowngf"), "dường");
        assert_eq!(telex("Vieetj"), "Việt");
    }

    #[test]
    fn telex_common_words() {
        for (keys, word) in [
            ("khoong", "không"),
            ("nhuwngx", "những"),
            ("quoocs", "quốc"),
            ("giuwax", "giữa"),
            ("nghieeng", "nghiêng"),
            ("ddeemf", "đềm"),
            ("thuyeets", "thuyết"),
            ("khuyeens", "khuyến"),
            ("ruowuj", "rượu"),
            ("yeeu", "yêu"),
            ("oachs", "oách"),
            ("cuwas", "cứa"),
            ("hieeur", "hiểu"),
            ("trawng", "trăng"),
            ("baan", "bân"),
            ("ghees", "ghế"),
            ("ddaay", "đây"),
            ("mooj", "mộ"),
            ("phuowngf", "phường"),
        ] {
            assert_eq!(telex(keys), word, "{keys}");
        }
    }

    #[test]
    fn telex_free_style_marks() {
        assert_eq!(telex("chungws"), "chứng");
        assert_eq!(telex("chungsw"), "chứng");
        assert_eq!(telex("truongw"), "trương");
        assert_eq!(telex("nguoiwf"), "người");
        assert_eq!(telex("tiengse"), "tiếng");
        assert_eq!(telex("vietej"), "việt");
        assert_eq!(telex("chana"), "chân");
        assert_eq!(telex("muoon"), "muôn");
        assert_eq!(telex("hoawcj"), "hoặc");
        assert_eq!(telex("muaw"), "mưa");
        assert_eq!(telex("guiwr"), "gửi");
        assert_eq!(telex("did"), "đi");
        assert_eq!(telex("duowcjd"), "được");
        assert_eq!(telex("dduwowcj"), "được");
    }

    #[test]
    fn telex_uo() {
        assert_eq!(telex("thuowr"), "thuở");
        assert_eq!(telex("truwong"), "trương");
        assert_eq!(telex("huow"), "hươ");
        assert_eq!(telex("nguwowif"), "người");
        assert_eq!(telex("muowon"), "muôn");
    }

    #[test]
    fn qu_and_gi() {
        assert_eq!(telex("quas"), "quá");
        assert_eq!(telex("giaf"), "già");
        assert_eq!(telex("quaan"), "quân");
        assert_eq!(telex("quawng"), "quăng");
        assert_eq!(telex("quyeenf"), "quyền");
        assert_eq!(telex("gieengs"), "giếng");
        assert_eq!(telex("gif"), "gì");
        assert_eq!(telex("gifn"), "gìn");
    }

    #[test]
    fn tone_position() {
        assert_eq!(telex("hoaf"), "hòa");
        assert_eq!(telex("thuys"), "thúy");
        assert_eq!(telex("muas"), "múa");
        assert_eq!(telex("tafi"), "tài");
        assert_eq!(telex("hoanf"), "hoàn");
        assert_eq!(telex("toasn"), "toán");
        assert_eq!(telex("ngoaif"), "ngoài");
        assert_eq!(telex("xoays"), "xoáy");
        assert_eq!(telex("khuyar"), "khuỷa");
    }

    #[test]
    fn classic_tone_position() {
        let mut engine = Engine::with_style(TypingStyle::Telex);
        engine.set_tone_placement(TonePlacement::Classic);
        assert_eq!(type_str(&mut engine, "hoaf"), "hoà");
        engine.reset();
        assert_eq!(type_str(&mut engine, "thuys"), "thuý");
    }

    #[test]
    fn spell_check_stops_marks_on_non_vietnamese() {
        assert_eq!(telex("class"), "class");
        assert_eq!(telex("kas"), "kas"); // k only goes with e, ê, i, y
        assert_eq!(telex("chuacx"), "chuacx"); // "uac" isn't a rhyme
        assert_eq!(telex("catf"), "catf"); // c/ch/p/t finals only take sắc, nặng
        assert_eq!(telex("hello"), "hello");
        assert_eq!(telex("facebook"), "facebook");
    }

    #[test]
    fn auto_restore_on_word_break() {
        assert_eq!(telex("windows "), "windows ");
        assert_eq!(telex("Usb "), "Usb ");
        assert_eq!(telex("as1 "), "as1 ");
        assert_eq!(telex("vieetj nam "), "việt nam ");
        assert_eq!(telex("chungws ta, "), "chứng ta, ");
        assert_eq!(telex("aaa "), "aa ");
    }

    #[test]
    fn auto_restore_keeps_valid_syllables() {
        // Real Vietnamese syllables are kept, even if meant as English.
        assert_eq!(telex("data "), "dât ");
        assert_eq!(telex("pwm "), "pưm ");
    }

    #[test]
    fn simple_telex_keeps_lone_w() {
        assert_eq!(typed(TypingStyle::SimpleTelex, "pwm "), "pwm ");
        assert_eq!(typed(TypingStyle::SimpleTelex, "www "), "www ");
        assert_eq!(typed(TypingStyle::SimpleTelex, "chungws"), "chứng");
        assert_eq!(typed(TypingStyle::SimpleTelex, "tuw"), "tư");
    }

    #[test]
    fn auto_restore_can_be_turned_off() {
        let mut engine = Engine::with_style(TypingStyle::Telex);
        engine.set_auto_restore(false);
        assert_eq!(type_str(&mut engine, "windows "), "ưindows ");
    }

    #[test]
    fn enter_ends_the_word_without_restoring() {
        let mut engine = Engine::with_style(TypingStyle::Telex);
        assert_eq!(type_str(&mut engine, "windows"), "ưindows");
        assert_eq!(engine.process(KeyEvent::WordBoundary), Action::PassThrough);
    }

    #[test]
    fn backspace_deletes_whole_letters() {
        let mut engine = Engine::with_style(TypingStyle::Telex);
        let mut screen = type_str(&mut engine, "vieetj");
        assert_eq!(screen, "việt");
        backspace(&mut engine, &mut screen);
        assert_eq!(screen, "việ");
        backspace(&mut engine, &mut screen);
        assert_eq!(screen, "vi");
    }

    #[test]
    fn backspace_moves_tone() {
        let mut engine = Engine::with_style(TypingStyle::Telex);
        let mut screen = type_str(&mut engine, "toasn");
        assert_eq!(screen, "toán");
        backspace(&mut engine, &mut screen);
        assert_eq!(screen, "tóa");
    }

    #[test]
    fn backspace_on_empty_word_passes_through() {
        let mut engine = Engine::new();
        assert_eq!(engine.process(KeyEvent::Backspace), Action::PassThrough);
    }

    #[test]
    fn tone_follows_the_syllable() {
        let mut engine = Engine::with_style(TypingStyle::Telex);
        assert_eq!(type_str(&mut engine, "toas"), "tóa");
        let mut screen = "tóa".to_string();
        apply(&mut screen, engine.process(KeyEvent::Char('n')), Some('n'));
        assert_eq!(screen, "toán");
    }

    #[test]
    fn vni_keys() {
        assert_eq!(vni("a6"), "â");
        assert_eq!(vni("o7"), "ơ");
        assert_eq!(vni("a8"), "ă");
        assert_eq!(vni("d9"), "đ");
        assert_eq!(vni("a1"), "á");
        assert_eq!(vni("a150"), "a");
        assert_eq!(vni("vie6t5"), "việt");
        assert_eq!(vni("chung71"), "chứng");
        assert_eq!(vni("nguoi72"), "người");
        assert_eq!(vni("duoc759"), "được");
        assert_eq!(vni("tieng61"), "tiếng");
    }

    #[test]
    fn english_mode_passes_through() {
        let mut engine = Engine::new();
        engine.set_mode(InputMode::English);
        assert_eq!(type_str(&mut engine, "vieetj"), "vieetj");
    }

    #[test]
    fn switching_mode_resets_the_word() {
        let mut engine = Engine::new();
        engine.process(KeyEvent::Char('a'));
        engine.set_mode(InputMode::English);
        engine.set_mode(InputMode::Vietnamese);
        assert_eq!(engine.process(KeyEvent::Char('a')), Action::PassThrough);
    }
}
