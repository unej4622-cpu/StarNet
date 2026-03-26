//! StarNet Input - Input event simulation abstraction layer.
//!
//! Provides a platform-agnostic `InputSimulator` trait with implementations
//! for Windows (SendInput API) and macOS (CGEventPost).

use async_trait::async_trait;
use starnet_core::{InputEvent, MouseButton};
use thiserror::Error;

/// Errors that can occur during input simulation.
#[derive(Debug, Error)]
pub enum InputError {
    #[error("failed to simulate mouse event: {0}")]
    MouseEventFailed(String),
    #[error("failed to simulate keyboard event: {0}")]
    KeyEventFailed(String),
    #[error("input simulation not available on this platform")]
    PlatformNotSupported,
    #[error("invalid input event: {0}")]
    InvalidEvent(String),
    #[error("{0}")]
    Other(String),
}

/// Trait for input event simulation.
#[async_trait]
pub trait InputSimulator: Send + Sync {
    async fn send_event(&self, event: InputEvent) -> Result<(), InputError>;
}

// ── Windows Implementation ──────────────────────────────────────────────────

#[cfg(target_os = "windows")]
mod windows_impl {
    use super::*;
    use windows::Win32::UI::Input::KeyboardAndMouse::*;
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

    fn mouse_button_flags(button: MouseButton, pressed: bool) -> MOUSE_EVENT_FLAGS {
        match (button, pressed) {
            (MouseButton::Left, true) => MOUSEEVENTF_LEFTDOWN,
            (MouseButton::Left, false) => MOUSEEVENTF_LEFTUP,
            (MouseButton::Right, true) => MOUSEEVENTF_RIGHTDOWN,
            (MouseButton::Right, false) => MOUSEEVENTF_RIGHTUP,
            (MouseButton::Middle, true) => MOUSEEVENTF_MIDDLEDOWN,
            (MouseButton::Middle, false) => MOUSEEVENTF_MIDDLEUP,
            (MouseButton::Back, true) => MOUSEEVENTF_XDOWN,
            (MouseButton::Back, false) => MOUSEEVENTF_XUP,
            (MouseButton::Forward, true) => MOUSEEVENTF_XDOWN,
            (MouseButton::Forward, false) => MOUSEEVENTF_XUP,
        }
    }

    fn x_button_data(button: MouseButton) -> u32 {
        match button {
            MouseButton::Back => 0x0001,
            MouseButton::Forward => 0x0002,
            _ => 0,
        }
    }

    fn make_mouse_input(mi: MOUSEINPUT) -> INPUT {
        INPUT {
            r#type: INPUT_TYPE(0), // INPUT_MOUSE
            Anonymous: INPUT_0 { mi },
        }
    }

    fn make_keyboard_input(ki: KEYBDINPUT) -> INPUT {
        INPUT {
            r#type: INPUT_TYPE(1), // INPUT_KEYBOARD
            Anonymous: INPUT_0 { ki },
        }
    }

    fn send_inputs(inputs: &[INPUT]) -> std::result::Result<(), InputError> {
        unsafe {
            let sent = SendInput(inputs, std::mem::size_of::<INPUT>() as i32);
            if sent != inputs.len() as u32 {
                return Err(InputError::MouseEventFailed(format!(
                    "SendInput: sent {sent}, expected {}", inputs.len()
                )));
            }
        }
        Ok(())
    }

    /// Windows input simulator using the SendInput API.
    pub struct WindowsInputSimulator;

    impl WindowsInputSimulator {
        pub fn new() -> Self {
            Self
        }

        fn dispatch_event(&self, event: InputEvent) -> std::result::Result<(), InputError> {
            match event {
                InputEvent::MouseMove { x, y } => {
                    let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
                    let screen_h = unsafe { GetSystemMetrics(SM_CYSCREEN) };

                    let mi = if screen_w > 0 && screen_h > 0 {
                        MOUSEINPUT {
                            dx: ((x / screen_w as f64) * 65535.0) as i32,
                            dy: ((y / screen_h as f64) * 65535.0) as i32,
                            dwFlags: MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK | MOUSEEVENTF_MOVE,
                            ..Default::default()
                        }
                    } else {
                        MOUSEINPUT {
                            dx: x as i32,
                            dy: y as i32,
                            dwFlags: MOUSEEVENTF_MOVE,
                            ..Default::default()
                        }
                    };
                    send_inputs(&[make_mouse_input(mi)])
                }

                InputEvent::MouseClick { button, x, y, pressed } => {
                    let flags = mouse_button_flags(button, pressed);
                    let mut mi = MOUSEINPUT {
                        dx: x as i32,
                        dy: y as i32,
                        dwFlags: flags | MOUSEEVENTF_VIRTUALDESK,
                        ..Default::default()
                    };
                    if matches!(button, MouseButton::Back | MouseButton::Forward) {
                        mi.mouseData = x_button_data(button) as u32;
                    }
                    send_inputs(&[make_mouse_input(mi)])
                }

                InputEvent::MouseScroll { x, y, delta_x, delta_y } => {
                    let mut inputs = Vec::new();

                    if delta_y != 0.0 {
                        let scroll = (-delta_y * 120.0) as i32;
                        let mi = MOUSEINPUT {
                            dx: x as i32,
                            dy: y as i32,
                            mouseData: scroll as u32,
                            dwFlags: MOUSEEVENTF_WHEEL | MOUSEEVENTF_VIRTUALDESK,
                            ..Default::default()
                        };
                        inputs.push(make_mouse_input(mi));
                    }

                    if delta_x != 0.0 {
                        let scroll = (-delta_x * 120.0) as i32;
                        let mi = MOUSEINPUT {
                            dx: x as i32,
                            dy: y as i32,
                            mouseData: scroll as u32,
                            dwFlags: MOUSEEVENTF_HWHEEL | MOUSEEVENTF_VIRTUALDESK,
                            ..Default::default()
                        };
                        inputs.push(make_mouse_input(mi));
                    }

                    if inputs.is_empty() {
                        Ok(())
                    } else {
                        send_inputs(&inputs)
                    }
                }

                InputEvent::KeyPress { key, modifiers } => {
                    let mut inputs = Vec::new();

                    // Modifiers down
                    Self::modifier_inputs(modifiers, true, &mut inputs);

                    // Key down
                    inputs.push(make_keyboard_input(KEYBDINPUT {
                        wVk: VIRTUAL_KEY(key as u16),
                        dwFlags: KEYBD_EVENT_FLAGS(0),
                        ..Default::default()
                    }));
                    // Key up
                    inputs.push(make_keyboard_input(KEYBDINPUT {
                        wVk: VIRTUAL_KEY(key as u16),
                        dwFlags: KEYEVENTF_KEYUP,
                        ..Default::default()
                    }));

                    // Modifiers up
                    Self::modifier_inputs(modifiers, false, &mut inputs);

                    send_inputs(&inputs)
                }

                InputEvent::KeyRelease { key, modifiers } => {
                    let mut inputs = Vec::new();

                    inputs.push(make_keyboard_input(KEYBDINPUT {
                        wVk: VIRTUAL_KEY(key as u16),
                        dwFlags: KEYEVENTF_KEYUP,
                        ..Default::default()
                    }));

                    Self::modifier_inputs(modifiers, false, &mut inputs);

                    send_inputs(&inputs)
                }
            }
        }

        fn modifier_inputs(modifiers: u32, pressed: bool, out: &mut Vec<INPUT>) {
            let mods = [
                (0x0001, VK_CONTROL),
                (0x0002, VK_SHIFT),
                (0x0004, VK_MENU),
                (0x0008, VK_LWIN),
            ];
            let flags = if pressed { KEYBD_EVENT_FLAGS(0) } else { KEYEVENTF_KEYUP };
            for (flag, vk) in mods {
                if modifiers & flag != 0 {
                    out.push(make_keyboard_input(KEYBDINPUT {
                        wVk: vk,
                        dwFlags: flags,
                        ..Default::default()
                    }));
                }
            }
        }
    }

    #[async_trait]
    impl InputSimulator for WindowsInputSimulator {
        async fn send_event(&self, event: InputEvent) -> std::result::Result<(), InputError> {
            tokio::task::block_in_place(|| self.dispatch_event(event))
        }
    }

    impl Default for WindowsInputSimulator {
        fn default() -> Self {
            Self::new()
        }
    }
}

#[cfg(target_os = "windows")]
pub use windows_impl::WindowsInputSimulator;

// ── Fallback for non-supported platforms ────────────────────────────────────

#[cfg(not(target_os = "windows"))]
pub struct StubInputSimulator;

#[cfg(not(target_os = "windows"))]
#[async_trait]
impl InputSimulator for StubInputSimulator {
    async fn send_event(&self, _event: InputEvent) -> std::result::Result<(), InputError> {
        Err(InputError::PlatformNotSupported)
    }
}
