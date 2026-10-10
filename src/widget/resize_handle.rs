// SPDX-License-Identifier: MPL-2.0

//! A draggable strip for resizing a neighboring element, such as the nav bar.

use crate::{Element, Renderer, Theme};

use iced_core::event::Event;
use iced_core::widget::{tree, Tree};
use iced_core::{
    layout, mouse,
    renderer::{self, Quad},
    Border, Clipboard, Layout, Length, Point, Rectangle, Renderer as _, Shadow, Shell, Size,
    Widget,
};

/// The width of the drag strip of a [`ResizeHandle`].
pub const HANDLE_WIDTH: f32 = 10.0;

#[derive(Default)]
struct DragState {
    /// Whether the user is currently dragging the handle.
    dragging: bool,
    /// Whether the handle has been dragged since the button was pressed.
    moved: bool,
    /// Cursor position, in window coordinates, where the drag started.
    start_x: f32,
    /// Width of the resized element when the drag started.
    start_width: f32,
}

/// A transparent strip that the user can drag to resize an adjacent element.
///
/// While dragging, the new width is emitted through [`ResizeHandle::new`]'s
/// callback; when the drag ends, an end message is emitted.
pub struct ResizeHandle<'a, Message> {
    width: f32,
    /// Width of the resized element, which a drag starts from.
    bar_width: f32,
    /// Produces the message emitted with the new width while dragging.
    on_resize: Box<dyn Fn(f32) -> Message + 'a>,
    /// Message emitted when the drag ends.
    on_resize_end: Message,
}

impl<'a, Message> ResizeHandle<'a, Message> {
    /// Creates a drag handle for an element of the given width.
    pub fn new(
        bar_width: f32,
        on_resize: impl Fn(f32) -> Message + 'a,
        on_resize_end: Message,
    ) -> Self {
        Self {
            width: HANDLE_WIDTH,
            bar_width,
            on_resize: Box::new(on_resize),
            on_resize_end,
        }
    }

    /// Sets the width of the drag strip.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
}

/// A transparent strip that the user can drag to resize an adjacent element.
pub fn resize_handle<'a, Message>(
    bar_width: f32,
    on_resize: impl Fn(f32) -> Message + 'a,
    on_resize_end: Message,
) -> ResizeHandle<'a, Message> {
    ResizeHandle::new(bar_width, on_resize, on_resize_end)
}

impl<Message: Clone> Widget<Message, crate::Theme, Renderer> for ResizeHandle<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<DragState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(DragState::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(self.width), Length::Fill)
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(Size::new(self.width, limits.max().height))
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let Event::Mouse(mouse_event) = event else {
            return;
        };

        let state = tree.state.downcast_mut::<DragState>();

        match *mouse_event {
            mouse::Event::ButtonPressed(mouse::Button::Left) => {
                if let Some(position) = cursor.position_over(layout.bounds()) {
                    state.dragging = true;
                    state.moved = false;
                    state.start_x = position.x;
                    state.start_width = self.bar_width;
                    shell.capture_event();
                }
            }
            mouse::Event::CursorMoved { position } => {
                if state.dragging {
                    state.moved = true;
                    let width = state.start_width + (position.x - state.start_x);
                    shell.publish((self.on_resize)(width));
                }
            }
            mouse::Event::ButtonReleased(mouse::Button::Left) => {
                if state.dragging {
                    state.dragging = false;
                    if state.moved {
                        shell.publish(self.on_resize_end.clone());
                    }
                }
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let is_dragging = tree.state.downcast_ref::<DragState>().dragging;
        if is_dragging || cursor.is_over(layout.bounds()) {
            mouse::Interaction::ResizingHorizontally
        } else {
            mouse::Interaction::None
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let is_dragging = tree.state.downcast_ref::<DragState>().dragging;
        if !is_dragging && !cursor.is_over(layout.bounds()) {
            return;
        }

        let bounds = layout.bounds();
        let line_width = 2.0;
        let inset: f32 = 8.0;
        let mut color: iced_core::Color = theme.cosmic().accent.base.into();
        if !is_dragging {
            color.a *= 0.6;
        }

        renderer.fill_quad(
            Quad {
                bounds: Rectangle::new(
                    Point::new(
                        // Keep the visible cue closer to the sidebar edge.
                        bounds.x,
                        bounds.y + inset.min(bounds.height / 2.0),
                    ),
                    Size::new(line_width, (bounds.height - inset * 2.0).max(0.0)),
                ),
                border: Border {
                    radius: 1.0.into(),
                    width: 0.0,
                    color,
                },
                shadow: Shadow::default(),
                snap: true,
            },
            color,
        );
    }
}

impl<'a, Message> From<ResizeHandle<'a, Message>> for Element<'a, Message>
where
    Message: 'static + Clone,
{
    fn from(handle: ResizeHandle<'a, Message>) -> Self {
        Element::new(handle)
    }
}
