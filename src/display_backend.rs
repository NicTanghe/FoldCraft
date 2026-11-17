//! Linux display-server selection for the native window.
//!
//! cssimpler's GPU viewport embeds a child X11 window and refuses Wayland
//! surfaces ("Linux GPU viewports currently require X11"). In a Wayland session
//! that also exposes XWayland through `DISPLAY`, removing the Wayland variables
//! before winit starts makes it pick the X11 backend instead. This module holds
//! that decision as a pure function so it can be tested without a display.

use std::ffi::OsString;

/// Set to a non-empty value to keep the Wayland variables untouched.
pub const KEEP_WAYLAND_ENV: &str = "FOLDCRAFT_KEEP_WAYLAND";

const WAYLAND_VARS: [&str; 2] = ["WAYLAND_DISPLAY", "WAYLAND_SOCKET"];

/// Wayland variables to remove so winit falls back to XWayland, or an empty
/// list when no change is needed or possible: no Wayland session, no X11
/// `DISPLAY` to fall back to, or the user opted out with [`KEEP_WAYLAND_ENV`].
pub fn xwayland_fallback(get: impl Fn(&str) -> Option<OsString>) -> Vec<&'static str> {
    let set = |name: &str| get(name).is_some_and(|value| !value.is_empty());
    if set(KEEP_WAYLAND_ENV) || !set("DISPLAY") {
        return Vec::new();
    }
    WAYLAND_VARS.into_iter().filter(|name| set(name)).collect()
}

/// Apply [`xwayland_fallback`] to this process on Linux and report what changed
/// on standard error. Does nothing on other platforms.
///
/// # Safety
///
/// Call before any other thread exists (before the rayon pool, the simulation
/// worker or the window are created): it mutates the process environment.
pub unsafe fn prefer_xwayland_for_native_window() {
    if !cfg!(target_os = "linux") {
        return;
    }
    let removed = xwayland_fallback(|name| std::env::var_os(name));
    if removed.is_empty() {
        return;
    }
    for name in &removed {
        // SAFETY: the caller guarantees the process is still single-threaded.
        unsafe { std::env::remove_var(name) };
    }
    eprintln!(
        "Using XWayland: the native viewport requires X11, so {} {} unset for this process. \
         Set {KEEP_WAYLAND_ENV}=1 to keep Wayland.",
        removed.join(" and "),
        if removed.len() == 1 { "was" } else { "were" },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let map: HashMap<String, OsString> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), OsString::from(v)))
            .collect();
        move |name| map.get(name).cloned()
    }

    #[test]
    fn wayland_session_with_xwayland_drops_the_wayland_variables() {
        assert_eq!(
            xwayland_fallback(env(&[("WAYLAND_DISPLAY", "wayland-0"), ("DISPLAY", ":0")])),
            ["WAYLAND_DISPLAY"]
        );
        assert_eq!(
            xwayland_fallback(env(&[
                ("WAYLAND_DISPLAY", "wayland-0"),
                ("WAYLAND_SOCKET", "3"),
                ("DISPLAY", ":0"),
            ])),
            ["WAYLAND_DISPLAY", "WAYLAND_SOCKET"]
        );
    }

    #[test]
    fn nothing_changes_without_an_x11_display_or_a_wayland_session() {
        assert!(xwayland_fallback(env(&[("WAYLAND_DISPLAY", "wayland-0")])).is_empty());
        assert!(
            xwayland_fallback(env(&[("WAYLAND_DISPLAY", "wayland-0"), ("DISPLAY", "")])).is_empty()
        );
        assert!(xwayland_fallback(env(&[("DISPLAY", ":0")])).is_empty());
        assert!(xwayland_fallback(env(&[("WAYLAND_DISPLAY", ""), ("DISPLAY", ":0")])).is_empty());
        assert!(xwayland_fallback(env(&[])).is_empty());
    }

    #[test]
    fn opting_out_keeps_wayland() {
        assert!(
            xwayland_fallback(env(&[
                ("WAYLAND_DISPLAY", "wayland-0"),
                ("DISPLAY", ":0"),
                (KEEP_WAYLAND_ENV, "1"),
            ]))
            .is_empty()
        );
        // An empty opt-out value does not count as set.
        assert_eq!(
            xwayland_fallback(env(&[
                ("WAYLAND_DISPLAY", "wayland-0"),
                ("DISPLAY", ":0"),
                (KEEP_WAYLAND_ENV, ""),
            ])),
            ["WAYLAND_DISPLAY"]
        );
    }
}
