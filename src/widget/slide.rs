// Copyright 2023 System76 <info@system76.com>
// SPDX-License-Identifier: MPL-2.0

//! Animated width transition for widgets that slide in from a window edge.
//!
//! Used by the context drawer and the nav bar. The content is always laid out at
//! its full width, so it does not reflow while the slide animates; the visible
//! region is clipped to a width interpolated between [`Slide::min_width`] and the
//! content's full width.

use crate::{Element, Renderer, Theme, anim};
use std::time::{Duration, Instant};

use iced_core::event::Event;
use iced_core::widget::{Operation, Tree, tree};
use iced_core::{
    Clipboard, Layout, Length, Rectangle, Renderer as _, Shell, Size, Vector, Widget, layout,
    mouse, overlay as iced_overlay, renderer, window,
};

pub(crate) const ANIMATION_DURATION: Duration = Duration::from_millis(200);

/// Animation state shared by a slide and, for overlay drawers, the overlay it opens.
#[derive(Default)]
pub(crate) struct SlideAnimation {
    animation: anim::State,
    target: bool,
    active: bool,
    last_layout_request: Option<Instant>,
}

impl SlideAnimation {
    pub(crate) fn progress(&mut self, target: bool, animating: bool) -> f32 {
        if target != self.target {
            if !animating {
                // Settle at the target, so a later transition animates from here
                self.animation = anim::State::default();
                self.active = false;
                self.target = target;
                return if target { 1.0 } else { 0.0 };
            }

            if self.active {
                self.animation.changed(ANIMATION_DURATION);
            } else {
                self.animation = anim::State::default();
                self.animation.changed(ANIMATION_DURATION);
            }

            self.target = target;
            self.active = true;
        }

        if !self.active {
            return if target { 1.0 } else { 0.0 };
        }

        let t = self.animation.t(ANIMATION_DURATION, self.target);
        let progress = anim::smootherstep(t);

        if (self.target && t >= 1.0) || (!self.target && t <= 0.0) {
            self.animation = anim::State::default();
            self.active = false;
        }

        progress
    }

    pub(crate) fn active(&self) -> bool {
        self.active
    }

    pub(crate) fn needs_layout_for(&mut self, redraw: Instant) -> bool {
        if self.last_layout_request == Some(redraw) {
            false
        } else {
            self.last_layout_request = Some(redraw);
            true
        }
    }
}

/// The visible width of a slide at the given progress.
fn visible_width(full_width: f32, min_width: f32, progress: f32) -> f32 {
    let min_width = min_width.min(full_width);
    min_width + (full_width - min_width) * progress
}

#[must_use]
pub(crate) struct Slide<'a, Message> {
    content: Element<'a, Message>,
    width: Option<f32>,
    min_width: f32,
    open: bool,
    animating: bool,
}

impl<'a, Message: Clone + 'static> Slide<'a, Message> {
    pub(crate) fn new(
        content: impl Into<Element<'a, Message>>,
        open: bool,
        animating: bool,
    ) -> Self {
        Self {
            content: content.into(),
            width: None,
            min_width: 0.0,
            open,
            animating,
        }
    }

    /// Sets the width occupied when fully open, instead of the content's own width.
    pub(crate) fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Width that remains visible while closed, revealing the leading edge of the content.
    pub(crate) fn min_width(mut self, min_width: f32) -> Self {
        self.min_width = min_width;
        self
    }
}

impl<Message: Clone> Widget<Message, crate::Theme, Renderer> for Slide<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<SlideAnimation>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(SlideAnimation::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(&mut [&mut self.content]);
    }

    fn size(&self) -> iced_core::Size<Length> {
        iced_core::Size::new(Length::Shrink, self.content.as_widget().size().height)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let progress = tree
            .state
            .downcast_mut::<SlideAnimation>()
            .progress(self.open, self.animating);

        let max_size = limits.max();
        let max_width = max_size.width.max(0.0);
        let open_width = self.width.unwrap_or(max_width).min(max_width);
        let limits = layout::Limits::new(Size::ZERO, Size::new(open_width, max_size.height));
        let content = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, &limits);
        let content_size = content.size();

        let full_width = self.width.unwrap_or(content_size.width).min(max_width);
        let width = visible_width(full_width, self.min_width, progress);

        layout::Node::with_children(Size::new(width, content_size.height), vec![content])
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if let Event::Window(window::Event::RedrawRequested(now)) = event {
            let animation = tree.state.downcast_mut::<SlideAnimation>();

            if animation.active() {
                shell.request_redraw();

                if animation.needs_layout_for(*now) {
                    shell.invalidate_layout();
                }
            }
        }

        if matches!(event, Event::Mouse(_) | Event::Touch(_)) && !cursor.is_over(layout.bounds()) {
            return;
        }

        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout.child(0),
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        if !cursor.is_over(layout.bounds()) {
            return mouse::Interaction::None;
        }

        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout.child(0),
            cursor,
            viewport,
            renderer,
        )
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation<()>,
    ) {
        self.content.as_widget_mut().operate(
            &mut tree.children[0],
            layout.child(0),
            renderer,
            operation,
        );
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        renderer_style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        renderer.with_layer(layout.bounds(), |renderer| {
            self.content.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                renderer_style,
                layout.child(0),
                cursor,
                viewport,
            );
        });
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<iced_overlay::Element<'b, Message, crate::Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout.child(0),
            renderer,
            viewport,
            translation,
        )
    }

    #[cfg(feature = "a11y")]
    fn a11y_nodes(
        &self,
        layout: Layout<'_>,
        state: &Tree,
        p: mouse::Cursor,
    ) -> iced_accessibility::A11yTree {
        self.content
            .as_widget()
            .a11y_nodes(layout.child(0), &state.children[0], p)
    }

    fn drag_destinations(
        &self,
        state: &Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        dnd_rectangles: &mut iced_core::clipboard::DndDestinationRectangles,
    ) {
        self.content.as_widget().drag_destinations(
            &state.children[0],
            layout.child(0),
            renderer,
            dnd_rectangles,
        );
    }
}

impl<'a, Message: 'a + Clone> From<Slide<'a, Message>> for Element<'a, Message> {
    fn from(widget: Slide<'a, Message>) -> Element<'a, Message> {
        Element::new(widget)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pretends the running animation started long ago.
    fn finish(animation: &mut SlideAnimation) {
        animation.animation.last_change = Some(Instant::now() - ANIMATION_DURATION * 2);
    }

    #[test]
    fn slide_animation_opens_and_closes() {
        let mut animation = SlideAnimation::default();

        // Opening starts hidden.
        assert_eq!(animation.progress(true, true), 0.0);
        assert!(animation.active());

        finish(&mut animation);
        assert_eq!(animation.progress(true, true), 1.0);
        assert!(!animation.active());

        // Closing starts open.
        assert_eq!(animation.progress(false, true), 1.0);
        assert!(animation.active());

        finish(&mut animation);
        assert_eq!(animation.progress(false, true), 0.0);
        assert!(!animation.active());
    }

    #[test]
    fn slide_animation_reverses_midway() {
        let mut animation = SlideAnimation::default();
        animation.progress(true, true);

        // Halfway through opening.
        animation.animation.last_change = Some(Instant::now() - ANIMATION_DURATION / 2);
        let halfway = animation.progress(true, true);
        assert!((0.4..=0.6).contains(&halfway), "halfway={halfway}");

        // Reversing towards closed continues from the current progress.
        let reversed = animation.progress(false, true);
        assert!((0.4..=0.6).contains(&reversed), "reversed={reversed}");
        assert!(animation.active());

        finish(&mut animation);
        assert_eq!(animation.progress(false, true), 0.0);
        assert!(!animation.active());
    }

    #[test]
    fn slide_animation_animates_from_a_settled_state() {
        let mut animation = SlideAnimation::default();

        // The slide starts open without animating, like a nav bar does.
        assert_eq!(animation.progress(true, false), 1.0);
        assert!(!animation.active());

        // Closing it afterwards starts from the open position.
        assert_eq!(animation.progress(false, true), 1.0);
        assert!(animation.active());

        finish(&mut animation);
        assert_eq!(animation.progress(false, true), 0.0);
        assert!(!animation.active());
    }

    #[test]
    fn slide_animation_stays_put_without_animation() {
        let mut animation = SlideAnimation::default();

        // Opening without animating jumps to the open position.
        assert_eq!(animation.progress(true, false), 1.0);
        assert!(!animation.active());

        // Closing without animating jumps to the hidden position.
        assert_eq!(animation.progress(false, false), 0.0);
        assert!(!animation.active());
    }

    #[test]
    fn visible_width_keeps_the_minimum_when_closed() {
        assert_eq!(visible_width(280.0, 7.0, 0.0), 7.0);
        assert_eq!(visible_width(280.0, 7.0, 1.0), 280.0);
        assert_eq!(visible_width(280.0, 7.0, 0.5), 143.5);

        // A slide that is narrower than its minimum never exceeds its content.
        assert_eq!(visible_width(4.0, 7.0, 0.5), 4.0);
    }
}
