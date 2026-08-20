//! Keep the game window out of the way during headless screenshot runs.
//!
//! miniquad creates a real window and, on macOS, activates the application
//! and brings that window to the front before any game code runs. That is
//! right for playing, but the screenshot harness launches the binary dozens
//! of times from a shell, and each launch stole focus from whatever the
//! developer was doing. Nothing in miniquad's `Conf` turns this off, so on
//! macOS the harness rewires the three Cocoa calls that cause it, before
//! `miniquad::start` makes them:
//!
//! - `-[NSApplication setActivationPolicy:]` is forced to *prohibited*, so the
//!   process gets no Dock icon or menu bar and can never become the frontmost
//!   application.
//! - `-[NSRunningApplication activateWithOptions:]` becomes a no-op, so the
//!   app delegate's "activate ignoring other apps" does nothing.
//! - `-[NSWindow makeKeyAndOrderFront:]` and `-[NSWindow orderFront:]` become
//!   no-ops, so the window is never shown on screen at all.
//!
//! The window and its OpenGL context still exist, so rendering, render
//! targets and `get_screen_data` all work exactly as on screen. Other
//! platforms are left alone.

#[cfg(target_os = "macos")]
mod platform {
    use macroquad::miniquad::native::apple::frameworks::{class, sel, sel_impl, Class, Object, Sel};
    use std::ffi::c_void;

    /// `IMP`: an untyped Objective-C method implementation, as the runtime
    /// hands them out.
    type Imp = unsafe extern "C" fn();

    /// Typed signatures of the methods being replaced. `BOOL` is returned as
    /// a byte, matching the Objective-C ABI on 64-bit Apple targets.
    type SetActivationPolicyFn = unsafe extern "C" fn(*mut Object, Sel, i64) -> u8;
    type ActivateWithOptionsFn = unsafe extern "C" fn(*mut Object, Sel, u64) -> u8;
    type OrderFrontFn = unsafe extern "C" fn(*mut Object, Sel, *mut Object);

    #[link(name = "objc", kind = "dylib")]
    extern "C" {
        fn class_getInstanceMethod(cls: *const Class, sel: Sel) -> *mut c_void;
        fn method_setImplementation(method: *mut c_void, imp: Imp) -> Imp;
    }

    /// `NSApplicationActivationPolicyProhibited`.
    const ACTIVATION_POLICY_PROHIBITED: i64 = 2;

    /// The real `-[NSApplication setActivationPolicy:]`, so the replacement
    /// can still apply a policy (just not the one it was asked for).
    static mut ORIGINAL_SET_ACTIVATION_POLICY: Option<Imp> = None;

    /// Replacement for `-[NSApplication setActivationPolicy:]`: whatever
    /// policy is requested, apply *prohibited* through the original method.
    unsafe extern "C" fn set_activation_policy_prohibited(
        this: *mut Object,
        cmd: Sel,
        _policy: i64,
    ) -> u8 {
        let Some(original) = ORIGINAL_SET_ACTIVATION_POLICY else {
            return 0;
        };
        let original: SetActivationPolicyFn = std::mem::transmute::<Imp, _>(original);

        original(this, cmd, ACTIVATION_POLICY_PROHIBITED)
    }

    /// Replacement for `-[NSRunningApplication activateWithOptions:]`.
    unsafe extern "C" fn activate_with_options_noop(_: *mut Object, _: Sel, _: u64) -> u8 {
        0
    }

    /// Replacement for `-[NSWindow makeKeyAndOrderFront:]` / `orderFront:`.
    unsafe extern "C" fn order_front_noop(_: *mut Object, _: Sel, _: *mut Object) {}

    /// Swap the implementation of an instance method, returning the original.
    unsafe fn replace_method(cls: *const Class, sel: Sel, imp: Imp) -> Option<Imp> {
        let method = class_getInstanceMethod(cls, sel);
        if method.is_null() {
            return None;
        }

        Some(method_setImplementation(method, imp))
    }

    pub fn install() {
        let set_activation_policy: SetActivationPolicyFn = set_activation_policy_prohibited;
        let activate_with_options: ActivateWithOptionsFn = activate_with_options_noop;
        let order_front: OrderFrontFn = order_front_noop;

        unsafe {
            ORIGINAL_SET_ACTIVATION_POLICY = replace_method(
                class!(NSApplication),
                sel!(setActivationPolicy:),
                std::mem::transmute::<SetActivationPolicyFn, Imp>(set_activation_policy),
            );
            replace_method(
                class!(NSRunningApplication),
                sel!(activateWithOptions:),
                std::mem::transmute::<ActivateWithOptionsFn, Imp>(activate_with_options),
            );
            for selector in [sel!(makeKeyAndOrderFront:), sel!(orderFront:)] {
                replace_method(
                    class!(NSWindow),
                    selector,
                    std::mem::transmute::<OrderFrontFn, Imp>(order_front),
                );
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    pub fn install() {}
}

/// Stop the window from appearing or taking focus for the rest of this
/// process. Must be called before the window is created, which for macroquad
/// means before `main` (the `#[macroquad::main]` wrapper creates it), so
/// callers use it from a `window_conf` function.
pub fn install() {
    platform::install();
}
