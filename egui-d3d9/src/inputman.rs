#![allow(dead_code)]
use std::time::Instant;

use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, Vec2};
use windows::Win32::{
        Foundation::{HWND, RECT},
        System::SystemServices::{MK_CONTROL, MK_SHIFT},
        UI::{
            Input::KeyboardAndMouse::{
                GetAsyncKeyState, VIRTUAL_KEY, VK_BACK, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END,
                VK_ESCAPE, VK_HOME, VK_INSERT, VK_LEFT, VK_LSHIFT, VK_NEXT, VK_PRIOR, VK_RETURN,
                VK_RIGHT, VK_SPACE, VK_TAB, VK_UP,
            },
            WindowsAndMessaging::{
                GetClientRect, KF_REPEAT, WHEEL_DELTA, WM_CHAR, WM_KEYDOWN, WM_KEYUP,
                WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDBLCLK, WM_MBUTTONDOWN,
                WM_MBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONDBLCLK,
                WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_XBUTTONDBLCLK,
                WM_XBUTTONDOWN, WM_XBUTTONUP, XBUTTON1, XBUTTON2,
            },
        },
    };

use crate::get_clipboard_text;

pub struct InputManager {
    hwnd: HWND,
    events: Vec<Event>,
    modifiers: Option<Modifiers>,
    start: Instant,
}

/// High-level overview of recognized `WndProc` messages.
#[repr(u8)]
pub enum InputResult {
    Unknown,
    MouseMove,
    MouseLeft,
    MouseRight,
    MouseMiddle,
    Character,
    Scroll,
    Zoom,
    Key,
}

impl InputResult {
    #[inline]
    pub const fn is_valid(&self) -> bool {
        !self.is_unknown()
    }

    #[inline]
    pub const fn is_unknown(&self) -> bool {
        matches!(*self, Self::Unknown)
    }
}

impl InputManager {
    pub fn new(hwnd: HWND) -> Self {
        Self {
            hwnd,
            events: vec![],
            modifiers: None,
            start: Instant::now()
        }
    }

    #[allow(clippy::too_many_lines)]
    pub fn process(&mut self, umsg: u32, wparam: usize, lparam: isize) -> InputResult {
        let w_high = (wparam >> 16) as u16;

        match umsg {
            WM_MOUSEMOVE => {
                self.alter_modifiers(get_mouse_modifiers(wparam));

                self.events.push(Event::PointerMoved(get_pos(lparam)));
                InputResult::MouseMove
            }
            WM_LBUTTONDOWN | WM_LBUTTONDBLCLK => {
                let modifiers = get_mouse_modifiers(wparam);
                self.alter_modifiers(modifiers);

                self.events.push(Event::PointerButton {
                    pos: get_pos(lparam),
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers,
                });
                InputResult::MouseLeft
            }
            WM_LBUTTONUP => {
                let modifiers = get_mouse_modifiers(wparam);
                self.alter_modifiers(modifiers);

                self.events.push(Event::PointerButton {
                    pos: get_pos(lparam),
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers,
                });
                InputResult::MouseLeft
            }
            WM_RBUTTONDOWN | WM_RBUTTONDBLCLK => {
                let modifiers = get_mouse_modifiers(wparam);
                self.alter_modifiers(modifiers);

                self.events.push(Event::PointerButton {
                    pos: get_pos(lparam),
                    button: PointerButton::Secondary,
                    pressed: true,
                    modifiers,
                });
                InputResult::MouseRight
            }
            WM_RBUTTONUP => {
                let modifiers = get_mouse_modifiers(wparam);
                self.alter_modifiers(modifiers);

                self.events.push(Event::PointerButton {
                    pos: get_pos(lparam),
                    button: PointerButton::Secondary,
                    pressed: false,
                    modifiers,
                });
                InputResult::MouseRight
            }
            WM_MBUTTONDOWN | WM_MBUTTONDBLCLK => {
                let modifiers = get_mouse_modifiers(wparam);
                self.alter_modifiers(modifiers);

                self.events.push(Event::PointerButton {
                    pos: get_pos(lparam),
                    button: PointerButton::Middle,
                    pressed: true,
                    modifiers,
                });
                InputResult::MouseMiddle
            }
            WM_MBUTTONUP => {
                let modifiers = get_mouse_modifiers(wparam);
                self.alter_modifiers(modifiers);

                self.events.push(Event::PointerButton {
                    pos: get_pos(lparam),
                    button: PointerButton::Middle,
                    pressed: false,
                    modifiers,
                });
                InputResult::MouseMiddle
            }
            WM_XBUTTONDOWN | WM_XBUTTONDBLCLK => {
                let modifiers = get_mouse_modifiers(wparam);
                self.alter_modifiers(modifiers);

                self.events.push(Event::PointerButton {
                    pos: get_pos(lparam),
                    button: if w_high & XBUTTON1 != 0 {
                        PointerButton::Extra1
                    } else if w_high & XBUTTON2 != 0 {
                        PointerButton::Extra2
                    } else {
                        unreachable!()
                    },
                    pressed: true,
                    modifiers,
                });
                InputResult::MouseMiddle
            }
            WM_XBUTTONUP => {
                let modifiers = get_mouse_modifiers(wparam);
                self.alter_modifiers(modifiers);

                self.events.push(Event::PointerButton {
                    pos: get_pos(lparam),
                    button: if w_high & XBUTTON1 != 0 {
                        PointerButton::Extra1
                    } else if w_high & XBUTTON2 != 0 {
                        PointerButton::Extra2
                    } else {
                        unreachable!()
                    },
                    pressed: false,
                    modifiers,
                });
                InputResult::MouseMiddle
            }
            WM_CHAR => {
                if let Some(ch) = char::from_u32(wparam as _) {
                    if !ch.is_control() {
                        self.events.push(Event::Text(ch.into()));
                    }
                }
                InputResult::Character
            }
            WM_MOUSEWHEEL => {
                let modifiers = get_mouse_modifiers(wparam);
                self.alter_modifiers(modifiers);

                let delta = w_high as i16 as f32 * 10. / WHEEL_DELTA as f32;

                if wparam & MK_CONTROL.0 as usize != 0 {
                    self.events
                        .push(Event::Zoom(if delta > 0. { 1.5 } else { 0.5 }));
                    InputResult::Zoom
                } else {
                    self.events.push(Event::MouseWheel {
                        delta: Vec2::new(0., delta),
                        unit: egui::MouseWheelUnit::Point,
                        modifiers,
                    });
                    InputResult::Scroll
                }
            }
            WM_MOUSEHWHEEL => {
                let modifiers = get_mouse_modifiers(wparam);
                self.alter_modifiers(modifiers);

                let delta = w_high as i16 as f32 * 10. / WHEEL_DELTA as f32;

                if wparam & MK_CONTROL.0 as usize != 0 {
                    self.events
                        .push(Event::Zoom(if delta > 0. { 1.5 } else { 0.5 }));
                    InputResult::Zoom
                } else {
                    self.events.push(Event::MouseWheel {
                        delta: Vec2::new(delta, 0.),
                        unit: egui::MouseWheelUnit::Point,
                        modifiers,
                    });
                    InputResult::Scroll
                }
            }
            msg @ (WM_KEYDOWN | WM_SYSKEYDOWN) => {
                let modifiers = get_key_modifiers(msg);
                self.modifiers = Some(modifiers);

                if let Some(key) = get_key(wparam) {
                    if key == Key::V && modifiers.ctrl {
                        if let Ok(clipboard) = get_clipboard_text() {
                            self.events.push(Event::Text(clipboard));
                        }
                    }

                    if key == Key::C && modifiers.ctrl {
                        self.events.push(Event::Copy);
                    }

                    if key == Key::X && modifiers.ctrl {
                        self.events.push(Event::Cut);
                    }

                    self.events.push(Event::Key {
                        pressed: true,
                        modifiers,
                        key,
                        repeat: lparam & (KF_REPEAT as isize) > 0,
                        physical_key: None,
                    });
                }
                InputResult::Key
            }
            msg @ (WM_KEYUP | WM_SYSKEYUP) => {
                let modifiers = get_key_modifiers(msg);
                self.modifiers = Some(modifiers);

                if let Some(key) = get_key(wparam) {
                    self.events.push(Event::Key {
                        pressed: false,
                        modifiers,
                        key,
                        repeat: false,
                        physical_key: None,
                    });
                }
                InputResult::Key
            }
            _ => InputResult::Unknown,
        }
    }

    fn alter_modifiers(&mut self, new: Modifiers) {
        if let Some(old) = self.modifiers.as_mut() {
            *old = new;
        }
    }

    pub fn collect_input(&mut self) -> RawInput {
        let time = self.get_system_time();
        RawInput {
            modifiers: self.modifiers.unwrap_or_default(),
            events: std::mem::take(&mut self.events),
            screen_rect: Some(self.get_screen_rect()),
            time: Some(time),
            //pixels_per_point: Some(1.),
            max_texture_side: None,
            predicted_dt: 1. / 60.,
            hovered_files: vec![],
            dropped_files: vec![],
            focused: true,
            ..Default::default()
        }
    }

    /// Returns time in seconds.
    pub fn get_system_time(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
        /*let mut time = 0;
        unsafe {
            NtQuerySystemTime(&mut time)
                .ok()
                .expect("Failed to get system time");
        }

        // dumb ass, read the docs. egui clearly says `in seconds`.
        // Shouldn't have wasted 3 days on this.
        // `NtQuerySystemTime` returns how many 100 nanosecond intervals
        // past since 1st Jan, 1601.
        (time as f64) / 10_000_000.*/
    }

    #[inline]
    pub fn get_screen_size(&self) -> Pos2 {
        let mut rect = RECT::default();
        unsafe {
            GetClientRect(self.hwnd, &mut rect).expect("Failed to GetClientRect()");
        }

        Pos2::new(
            (rect.right - rect.left) as f32,
            (rect.bottom - rect.top) as f32,
        )
    }

    #[inline]
    pub fn get_screen_rect(&self) -> Rect {
        Rect {
            min: Pos2::ZERO,
            max: self.get_screen_size(),
        }
    }
}

const fn get_pos(lparam: isize) -> Pos2 {
    let x = (lparam & 0xFFFF) as i16 as f32;
    let y = ((lparam >> 16) & 0xFFFF) as i16 as f32;

    Pos2::new(x, y)
}

const fn get_mouse_modifiers(wparam: usize) -> Modifiers {
    Modifiers {
        alt: false,
        ctrl: (wparam & MK_CONTROL.0 as usize) != 0,
        shift: (wparam & MK_SHIFT.0 as usize) != 0,
        mac_cmd: false,
        command: (wparam & MK_CONTROL.0 as usize) != 0,
    }
}

fn get_key_modifiers(msg: u32) -> Modifiers {
    let ctrl = unsafe { GetAsyncKeyState(VK_CONTROL.0 as _) != 0 };
    let shift = unsafe { GetAsyncKeyState(VK_LSHIFT.0 as _) != 0 };

    Modifiers {
        alt: msg == WM_SYSKEYDOWN,
        mac_cmd: false,
        command: ctrl,
        shift,
        ctrl,
    }
}

fn get_key(wparam: usize) -> Option<Key> {
    Some(match wparam {
        0x30..=0x39 => Key::ALL[(wparam as u8 - 0x1F) as usize],
        0x41..=0x5A => Key::ALL[(wparam as u8 - 0x26) as usize],
        0x70..=0x83 => Key::ALL[(wparam as u8 - 0x3b) as usize],
        _ => match VIRTUAL_KEY(wparam as u16) {
            VK_DOWN => Key::ArrowDown,
            VK_LEFT => Key::ArrowLeft,
            VK_RIGHT => Key::ArrowRight,
            VK_UP => Key::ArrowUp,
            VK_ESCAPE => Key::Escape,
            VK_TAB => Key::Tab,
            VK_BACK => Key::Backspace,
            VK_RETURN => Key::Enter,
            VK_SPACE => Key::Space,
            VK_INSERT => Key::Insert,
            VK_DELETE => Key::Delete,
            VK_HOME => Key::Home,
            VK_END => Key::End,
            VK_PRIOR => Key::PageUp,
            VK_NEXT => Key::PageDown,
            _ => {
                return None;
            }
        },
    })
}
