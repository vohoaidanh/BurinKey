//! Synthesizes keystrokes via `SendInput`.
//!
//! Every event we inject is tagged with [`OUR_INJECTED_MARKER`] in
//! `dwExtraInfo` so [`crate::hook`] can recognize and ignore it instead of
//! feeding it back into the engine, which would otherwise loop forever.

use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, VIRTUAL_KEY, VK_BACK,
};

pub const OUR_INJECTED_MARKER: usize = 0x5649_4D45; // "VIME"

/// Deletes `backspaces` characters to the left of the caret, then types `insert`.
pub fn replace_text(backspaces: u8, insert: &str) {
    let utf16_len = insert.encode_utf16().count();
    let mut inputs = Vec::with_capacity(backspaces as usize * 2 + utf16_len * 2);

    for _ in 0..backspaces {
        inputs.push(key_event(VK_BACK, KEYBD_EVENT_FLAGS(0)));
        inputs.push(key_event(VK_BACK, KEYEVENTF_KEYUP));
    }

    for unit in insert.encode_utf16() {
        inputs.push(unicode_event(unit, KEYBD_EVENT_FLAGS(0)));
        inputs.push(unicode_event(unit, KEYEVENTF_KEYUP));
    }

    send(&inputs);
}

fn send(inputs: &[INPUT]) {
    if inputs.is_empty() {
        return;
    }
    // SAFETY: `inputs` is a valid, non-empty slice of `INPUT` for the duration of the call.
    unsafe {
        SendInput(inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

fn key_event(vk: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: OUR_INJECTED_MARKER,
            },
        },
    }
}

/// A single UTF-16 code unit injected as Unicode input (no virtual-key mapping,
/// so it works regardless of the active keyboard layout).
fn unicode_event(utf16_unit: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: utf16_unit,
                dwFlags: KEYEVENTF_UNICODE | flags,
                time: 0,
                dwExtraInfo: OUR_INJECTED_MARKER,
            },
        },
    }
}
