// Copyright 2026 System76 <info@system76.com>
// SPDX-License-Identifier: MPL-2.0

//! Window helpers.

use crate::Task;
use crate::iced::window;

/// Returns the window identifier of the window.
///
/// The identifier can be passed to a portal dialog (e.g. the file chooser) so
/// that it is shown as a child of the window.
#[cfg(xdg_portal)]
pub fn identifier(id: window::Id) -> Task<Option<crate::dialog::ashpd::WindowIdentifier>> {
    #[cfg(wayland_platform)]
    return identifier_wayland(id);

    #[cfg(not(wayland_platform))]
    return identifier_x11(id);
}

#[cfg(all(xdg_portal, wayland_platform))]
fn identifier_wayland(
    id: window::Id,
) -> Task<Option<crate::dialog::ashpd::WindowIdentifier>> {
    use crate::iced::window::raw_window_handle::{
        HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle,
    };

    #[derive(Clone, Copy)]
    enum Display {
        Wayland,
        X11(u64),
        Unsupported,
    }

    window::run(id, move |window| {
        if let Ok(display) = window.display_handle()
            && matches!(display.as_raw(), RawDisplayHandle::Wayland(_))
        {
            return Display::Wayland;
        }

        match window.window_handle().map(|handle| handle.as_raw()) {
            Ok(RawWindowHandle::Xlib(window)) => Display::X11(window.window as u64),
            Ok(RawWindowHandle::Xcb(window)) => Display::X11(window.window.get().into()),
            _ => Display::Unsupported,
        }
    })
    .then(move |display| match display {
        Display::Wayland => {
            iced_winit::platform_specific::commands::dialog::window_surface(id).then(|surface| {
                crate::task::future(async move {
                    match surface {
                        Some(surface) => {
                            crate::dialog::ashpd::WindowIdentifier::from_wayland(&surface).await
                        }
                        None => None,
                    }
                })
            })
        }
        Display::X11(xid) => crate::task::future(async move {
            Some(crate::dialog::ashpd::WindowIdentifier::from_xid(xid))
        }),
        Display::Unsupported => Task::none(),
    })
}

#[cfg(all(xdg_portal, not(wayland_platform)))]
fn identifier_x11(id: window::Id) -> Task<Option<crate::dialog::ashpd::WindowIdentifier>> {
    use crate::iced::window::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    window::run(id, move |window| {
        match window.window_handle().map(|handle| handle.as_raw()) {
            Ok(RawWindowHandle::Xlib(window)) => Some(window.window as u64),
            Ok(RawWindowHandle::Xcb(window)) => Some(window.window.get().into()),
            _ => None,
        }
    })
    .then(|xid| {
        crate::task::future(
            async move { xid.map(crate::dialog::ashpd::WindowIdentifier::from_xid) },
        )
    })
}

/// Imports `parent` as the parent of the window and marks it as a modal dialog.
///
/// `parent` is an xdg-foreign handle as passed to portals, e.g.
/// `wayland:<handle>`. The window becomes modal once it is created.
#[cfg(wayland_platform)]
pub fn set_dialog(id: window::Id, parent: Option<String>, modal: bool) -> Task<()> {
    use iced_winit::platform_specific::commands::dialog;

    Task::batch([
        dialog::set_parent(id, parent),
        dialog::set_modal(id, modal),
    ])
}

/// No-op on platforms without Wayland dialog support.
#[cfg(not(wayland_platform))]
pub fn set_dialog(_id: window::Id, _parent: Option<String>, _modal: bool) -> Task<()> {
    Task::none()
}
