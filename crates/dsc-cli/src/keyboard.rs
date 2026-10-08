//! The keyboard's inputs, as distinct from any panel's.
//!
//! A panel's report says nothing about the keyboard: an MCDU key pressed with
//! Ctrl held sends the same bytes as without. So what the keyboard holds is
//! asked of Windows on its own, and belongs to the keyboard, not to any panel
//! it is used with. Which modifier means what is a setting, so the type lives
//! with the settings in `dsc-config`; asking Windows about it lives here.

use dsc_config::settings::Modifier;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_MENU, VK_SHIFT,
};
use windows_sys::Win32::UI::Input::{
    GetRawInputData, RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER,
    RIDEV_INPUTSINK, RIDEV_REMOVE, RID_INPUT, RIM_TYPEKEYBOARD,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, DispatchMessageW, GetMessageW, PostMessageW, HWND_MESSAGE, MSG,
    RI_KEY_BREAK, WM_APP, WM_INPUT,
};

/// Whether a modifier is down on the keyboard right now, whichever window has
/// focus. Asked at the moment a panel key goes down, so nothing polls it.
///
/// Left and right are one key: the generic virtual keys answer for either
/// side, which is the point, as nobody should have to remember which Ctrl.
pub fn held(m: Modifier) -> bool {
    let key = match m {
        Modifier::Ctrl => VK_CONTROL,
        Modifier::Shift => VK_SHIFT,
        Modifier::Alt => VK_MENU,
    };
    // The high bit is the key's state now; the low bit is only whether it
    // was pressed since some earlier call, which is no use here.
    (unsafe { GetAsyncKeyState(key as i32) }) < 0
}

/// Every modifier down right now.
pub fn held_now() -> Vec<Modifier> {
    Modifier::ALL.into_iter().filter(|m| held(*m)).collect()
}

/// Keystrokes as they happen, whichever window has focus, by Raw Input.
///
/// Raw Input hands this process a copy of each keystroke and nothing more: it
/// cannot hold one up or keep it from DCS, unlike a keyboard hook, which every
/// keystroke on the machine would wait on. A press is a message, so none is
/// missed between looks, and nothing wakes while no key moves.
///
/// Belongs to the thread that made it: the messages come to that thread's
/// queue, and [`next`](Keyboard::next) is asked there.
pub struct Keyboard {
    /// A message-only window, which is where Windows sends the copies.
    window: HWND,
}

/// What [`Keyboard::next`] found.
pub enum Typed {
    /// A key went down or came up. `vkey` is its virtual key, which for a
    /// digit in the row above the letters is the digit's character.
    Key { vkey: u16, down: bool },
    /// Anything else, such as a [`waker`](Keyboard::waker) asking the thread
    /// to look up.
    Other,
    /// The queue is closed; stop.
    Closed,
}

/// A message to the keyboard's thread to look up from waiting, from any
/// thread.
const LOOK_UP: u32 = WM_APP;

impl Keyboard {
    pub fn new() -> std::io::Result<Keyboard> {
        // A built-in class, so there is no window procedure of ours to
        // register: the messages are read off the queue before it sees them.
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        let window = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                std::ptr::null(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        if window.is_null() {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Keyboard { window })
    }

    /// Start or stop receiving keystrokes. Stopped, nothing arrives and the
    /// thread sleeps until a waker calls.
    pub fn listen(&self, on: bool) -> std::io::Result<()> {
        let device = RAWINPUTDEVICE {
            // Generic desktop, keyboard.
            usUsagePage: 0x01,
            usUsage: 0x06,
            dwFlags: if on { RIDEV_INPUTSINK } else { RIDEV_REMOVE },
            hwndTarget: if on {
                self.window
            } else {
                std::ptr::null_mut()
            },
        };
        let size = std::mem::size_of::<RAWINPUTDEVICE>() as u32;
        if unsafe { RegisterRawInputDevices(&device, 1, size) } == 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }

    /// Something any thread can call to have [`next`](Keyboard::next) return.
    pub fn waker(&self) -> impl Fn() + Send {
        // A window handle is only a number to post to; it is not touched.
        let window = self.window as usize;
        move || unsafe {
            PostMessageW(window as HWND, LOOK_UP, 0, 0);
        }
    }

    /// Wait for the next message and say what it was.
    pub fn next(&self) -> Typed {
        let mut msg: MSG = unsafe { std::mem::zeroed() };
        if unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) } <= 0 {
            return Typed::Closed;
        }
        let typed = if msg.message == WM_INPUT {
            read_key(msg.lParam as HRAWINPUT)
        } else {
            Typed::Other
        };
        // Lets Windows free what it kept for the input message.
        unsafe { DispatchMessageW(&msg) };
        typed
    }
}

impl Drop for Keyboard {
    fn drop(&mut self) {
        let _ = self.listen(false);
        unsafe { DestroyWindow(self.window) };
    }
}

fn read_key(input: HRAWINPUT) -> Typed {
    let mut raw: RAWINPUT = unsafe { std::mem::zeroed() };
    let mut size = std::mem::size_of::<RAWINPUT>() as u32;
    let read = unsafe {
        GetRawInputData(
            input,
            RID_INPUT,
            (&mut raw as *mut RAWINPUT).cast(),
            &mut size,
            std::mem::size_of::<RAWINPUTHEADER>() as u32,
        )
    };
    if read == u32::MAX || raw.header.dwType != RIM_TYPEKEYBOARD {
        return Typed::Other;
    }
    let key = unsafe { raw.data.keyboard };
    Typed::Key {
        vkey: key.VKey,
        down: u32::from(key.Flags) & RI_KEY_BREAK == 0,
    }
}
