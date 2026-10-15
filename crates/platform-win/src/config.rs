//! Settings that survive a restart, kept in
//! `%APPDATA%\BurinSmartKey\config.toml` as plain `key = value` lines that
//! are easy to read and edit by hand.
//!
//! The file is written after every change (all changes go through
//! `tray::refresh`). Unknown keys and bad values fall back to the defaults,
//! so a hand-edited file can never stop the app from starting.

use std::cell::Cell;
use std::path::PathBuf;

use engine::{Engine, InputMode, TonePlacement, TypingStyle};

use crate::hook;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    pub mode: InputMode,
    pub style: TypingStyle,
    pub tone: TonePlacement,
    pub auto_restore: bool,
    pub macros_enabled: bool,
    pub macros_in_english: bool,
    pub show_panel_on_start: bool,
}

impl Default for Config {
    fn default() -> Self {
        let engine = Engine::new();
        Self {
            mode: engine.mode(),
            style: engine.style(),
            tone: engine.tone_placement(),
            auto_restore: engine.auto_restore(),
            macros_enabled: engine.macros_enabled(),
            macros_in_english: engine.macros_in_english(),
            show_panel_on_start: true,
        }
    }
}

impl Config {
    pub fn parse(text: &str) -> Self {
        let mut config = Self::default();
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim().trim_matches('"');
            match key.trim() {
                "vietnamese" => {
                    if let Some(on) = parse_bool(value) {
                        config.mode = if on {
                            InputMode::Vietnamese
                        } else {
                            InputMode::English
                        };
                    }
                }
                "style" => match value {
                    "telex" => config.style = TypingStyle::Telex,
                    "simple-telex" => config.style = TypingStyle::SimpleTelex,
                    "vni" => config.style = TypingStyle::Vni,
                    _ => {}
                },
                "tone" => match value {
                    "modern" => config.tone = TonePlacement::Modern,
                    "classic" => config.tone = TonePlacement::Classic,
                    _ => {}
                },
                "auto_restore" => {
                    config.auto_restore = parse_bool(value).unwrap_or(config.auto_restore)
                }
                "macros" => {
                    config.macros_enabled = parse_bool(value).unwrap_or(config.macros_enabled)
                }
                "macros_in_english" => {
                    config.macros_in_english = parse_bool(value).unwrap_or(config.macros_in_english)
                }
                "show_panel_on_start" => {
                    config.show_panel_on_start =
                        parse_bool(value).unwrap_or(config.show_panel_on_start)
                }
                _ => {}
            }
        }
        config
    }

    pub fn to_text(self) -> String {
        let style = match self.style {
            TypingStyle::Telex => "telex",
            TypingStyle::SimpleTelex => "simple-telex",
            TypingStyle::Vni => "vni",
        };
        let tone = match self.tone {
            TonePlacement::Modern => "modern",
            TonePlacement::Classic => "classic",
        };
        format!(
            "# Burin Smart Key settings (saved by the app on every change)\n\
             vietnamese = {}\n\
             style = \"{style}\"  # telex | simple-telex | vni\n\
             tone = \"{tone}\"  # modern (tone on o: hoa) | classic (tone on a)\n\
             auto_restore = {}\n\
             macros = {}\n\
             macros_in_english = {}\n\
             show_panel_on_start = {}\n",
            self.mode == InputMode::Vietnamese,
            self.auto_restore,
            self.macros_enabled,
            self.macros_in_english,
            self.show_panel_on_start,
        )
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// The live configuration
// ---------------------------------------------------------------------------

thread_local! {
    /// Settings that live outside the engine.
    static SHOW_PANEL_ON_START: Cell<bool> = const { Cell::new(true) };
}

fn path() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    Some(
        PathBuf::from(appdata)
            .join("BurinSmartKey")
            .join("config.toml"),
    )
}

/// Reads the saved settings (defaults if there is no file yet) and applies
/// them to the engine. Returns them so startup can honour the rest.
pub fn load() -> Config {
    let config = path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|text| Config::parse(&text))
        .unwrap_or_default();
    hook::with_engine(|e| {
        e.set_mode(config.mode);
        e.set_style(config.style);
        e.set_tone_placement(config.tone);
        e.set_auto_restore(config.auto_restore);
        e.set_macros_enabled(config.macros_enabled);
        e.set_macros_in_english(config.macros_in_english);
    });
    SHOW_PANEL_ON_START.with(|c| c.set(config.show_panel_on_start));
    config
}

pub fn current() -> Config {
    hook::with_engine(|e| Config {
        mode: e.mode(),
        style: e.style(),
        tone: e.tone_placement(),
        auto_restore: e.auto_restore(),
        macros_enabled: e.macros_enabled(),
        macros_in_english: e.macros_in_english(),
        show_panel_on_start: SHOW_PANEL_ON_START.with(Cell::get),
    })
}

pub fn set_show_panel_on_start(on: bool) {
    SHOW_PANEL_ON_START.with(|c| c.set(on));
}

/// Writes the current settings if they differ from what is on disk.
/// Failures are ignored: settings just won't persist (e.g. read-only
/// profile), which must never break typing.
pub fn save() {
    let Some(path) = path() else {
        return;
    };
    let text = current().to_text();
    if std::fs::read_to_string(&path).is_ok_and(|old| old == text) {
        return;
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // Write then rename, so a crash mid-write can't leave a half file.
    let tmp = path.with_extension("toml.tmp");
    if std::fs::write(&tmp, text).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let config = Config {
            mode: InputMode::English,
            style: TypingStyle::SimpleTelex,
            tone: TonePlacement::Classic,
            auto_restore: false,
            macros_enabled: false,
            macros_in_english: true,
            show_panel_on_start: false,
        };
        assert_eq!(Config::parse(&config.to_text()), config);
    }

    #[test]
    fn bad_or_missing_values_use_defaults() {
        let config = Config::parse("style = \"dvorak\"\nauto_restore = maybe\nnonsense\n");
        assert_eq!(config, Config::default());
        assert_eq!(Config::parse(""), Config::default());
    }

    #[test]
    fn hand_edited_file_with_comments_and_spaces() {
        let config = Config::parse("  style=vni   # my choice\n vietnamese = false\n");
        assert_eq!(config.style, TypingStyle::Vni);
        assert_eq!(config.mode, InputMode::English);
    }
}
