#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaControl {
    Previous,
    PlayPause,
    Next,
}

pub fn virtual_key(control: MediaControl) -> u16 {
    match control {
        MediaControl::Previous => {
            windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_MEDIA_PREV_TRACK
        }
        MediaControl::PlayPause => {
            windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_MEDIA_PLAY_PAUSE
        }
        MediaControl::Next => windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_MEDIA_NEXT_TRACK,
    }
}

pub fn send(control: MediaControl) -> Result<(), String> {
    use std::mem::size_of;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
    };

    let key = virtual_key(control);
    let inputs = [
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: key,
                    ..Default::default()
                },
            },
        },
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: key,
                    dwFlags: KEYEVENTF_KEYUP,
                    ..Default::default()
                },
            },
        },
    ];

    let sent = unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            size_of::<INPUT>() as i32,
        )
    };

    if sent == inputs.len() as u32 {
        Ok(())
    } else {
        Err(format!(
            "Windows rejected the media control input (sent {sent} of {})",
            inputs.len()
        ))
    }
}

pub fn previous() -> Result<(), String> {
    send(MediaControl::Previous)
}

pub fn play_pause() -> Result<(), String> {
    send(MediaControl::PlayPause)
}

pub fn next() -> Result<(), String> {
    send(MediaControl::Next)
}

#[cfg(test)]
mod tests {
    use super::{virtual_key, MediaControl};

    #[test]
    fn maps_controls_to_windows_media_virtual_keys() {
        assert_eq!(virtual_key(MediaControl::Previous), 0xB1);
        assert_eq!(virtual_key(MediaControl::PlayPause), 0xB3);
        assert_eq!(virtual_key(MediaControl::Next), 0xB0);
    }
}
