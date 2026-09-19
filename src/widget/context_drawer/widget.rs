// Copyright 2023 System76 <info@system76.com>
// SPDX-License-Identifier: MPL-2.0

use super::ANIMATION_DURATION;
use super::overlay::Overlay;
use crate::anim;
use crate::widget::{self, LayerContainer, button, column, container, icon, row, scrollable, text};
use crate::{Apply, Element, Renderer, Theme, fl};
use std::borrow::Cow;
use std::time::Instant;

use iced_core::Renderer as _;
use iced_core::event::Event;
use iced_core::widget::{Operation, Tree, tree};
use iced_core::{
    Alignment, Clipboard, Layout, Length, Rectangle, Shell, Vector, Widget, layout, mouse,
    overlay as iced_overlay, renderer, window,
};

pub(super) struct DrawerAnimation {
    animation: anim::State,
    target: bool,
    active: bool,
    last_layout_request: Option<Instant>,
}

impl Default for DrawerAnimation {
    fn default() -> Self {
        Self {
            animation: anim::State::default(),
            target: false,
            active: false,
            last_layout_request: None,
        }
    }
}

impl DrawerAnimation {
    pub(super) fn progress(&mut self, target: bool, animating: bool) -> f32 {
        if animating && target != self.target {
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

    pub(super) fn active(&self) -> bool {
        self.active
    }

    pub(super) fn needs_layout_for(&mut self, redraw: Instant) -> bool {
        if self.last_layout_request == Some(redraw) {
            false
        } else {
            self.last_layout_request = Some(redraw);
            true
        }
    }
}

#[must_use]
pub struct ContextDrawer<'a, Message> {
    id: Option<iced_core::widget::Id>,
    content: Element<'a, Message>,
    drawer: Element<'a, Message>,
    on_close: Option<Message>,
    open: bool,
    animating: bool,
}

impl<'a, Message: Clone + 'static> ContextDrawer<'a, Message> {
    pub fn new_inner<Drawer>(
        title: Option<Cow<'a, str>>,
        actions: Option<Element<'a, Message>>,
        header: Option<Element<'a, Message>>,
        footer: Option<Element<'a, Message>>,
        drawer: Drawer,
        on_close: Message,
        max_width: f32,
        open: bool,
        animating: bool,
    ) -> Element<'a, Message>
    where
        Drawer: Into<Element<'a, Message>>,
    {
        Self::new_inner_overlay(
            title, actions, header, footer, drawer, on_close, max_width, open, animating, false,
        )
    }

    pub fn new_inner_overlay<Drawer>(
        title: Option<Cow<'a, str>>,
        actions: Option<Element<'a, Message>>,
        header: Option<Element<'a, Message>>,
        footer: Option<Element<'a, Message>>,
        drawer: Drawer,
        on_close: Message,
        max_width: f32,
        open: bool,
        animating: bool,
        overlay: bool,
    ) -> Element<'a, Message>
    where
        Drawer: Into<Element<'a, Message>>,
    {
        #[inline(never)]
        fn inner<'a, Message: Clone + 'static>(
            title: Option<Cow<'a, str>>,
            actions_opt: Option<Element<'a, Message>>,
            header_opt: Option<Element<'a, Message>>,
            footer_opt: Option<Element<'a, Message>>,
            drawer: Element<'a, Message>,
            on_close: Message,
            max_width: f32,
            open: bool,
            animating: bool,
            overlay: bool,
        ) -> Element<'a, Message> {
            let cosmic_theme::Spacing {
                space_xxs,
                space_s,
                space_m,
                space_l,
                ..
            } = crate::theme::spacing();

            let horizontal_padding = if max_width < 392.0 { space_s } else { space_l };

            let (actions_slot, column_title) = if let Some(actions) = actions_opt {
                let actions = actions
                    .apply(container)
                    .width(Length::Fill)
                    .apply(Element::from);
                let title = title.map(|title| text::title4(title).width(Length::Fill));
                (actions, title)
            } else {
                let title = title
                    .map(|title| text::title4(title).width(Length::Fill).apply(Element::from))
                    .unwrap_or_else(|| widget::space::horizontal().apply(Element::from));
                (title, None)
            };

            let header_row = row::with_capacity(2).push(actions_slot).push(
                button::text(fl!("close"))
                    .trailing_icon(icon::from_name("go-next-symbolic"))
                    .on_press(on_close),
            );
            let header = column::with_capacity(3)
                .align_x(Alignment::Center)
                .padding([space_m, horizontal_padding])
                .spacing(space_m)
                .push(header_row)
                .push_maybe(column_title)
                .push_maybe(header_opt);
            let footer = footer_opt.map(|element| {
                container(element)
                    .align_y(Alignment::Center)
                    .padding([space_xxs, horizontal_padding])
            });
            let pane = column::with_capacity(3)
                .push(header)
                .push(
                    container(drawer)
                        .padding([
                            0,
                            horizontal_padding,
                            if footer.is_some() { 0 } else { space_l },
                            horizontal_padding,
                        ])
                        .apply(scrollable)
                        .height(Length::Fill),
                )
                .push_maybe(footer);

            // XXX new limits do not exactly handle the max width well for containers
            // XXX this is a hack to get around that
            let root = container(
                LayerContainer::new(pane)
                    .layer(cosmic_theme::Layer::Primary)
                    .class(crate::style::Container::ContextDrawer {
                        transparent: !overlay,
                    })
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .max_width(max_width),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::End);

            if overlay {
                root.into()
            } else {
                Slide::new(root, max_width, open, animating).into()
            }
        }

        inner(
            title,
            actions,
            header,
            footer,
            drawer.into(),
            on_close,
            max_width,
            open,
            animating,
            overlay,
        )
    }

    /// Creates an empty [`ContextDrawer`].
    pub fn new<Content, Drawer>(
        title: Option<Cow<'a, str>>,
        actions: Option<Element<'a, Message>>,
        header: Option<Element<'a, Message>>,
        footer: Option<Element<'a, Message>>,
        content: Content,
        drawer: Drawer,
        on_close: Message,
        max_width: f32,
    ) -> Self
    where
        Content: Into<Element<'a, Message>>,
        Drawer: Into<Element<'a, Message>>,
    {
        let drawer = Self::new_inner_overlay(
            title, actions, header, footer, drawer, on_close, max_width, true, false, true,
        );

        ContextDrawer {
            id: None,
            content: content.into(),
            drawer,
            on_close: None,
            open: true,
            animating: false,
        }
    }

    /// Sets the [`Id`] of the [`ContextDrawer`].
    #[inline]
    pub fn id(mut self, id: iced_core::widget::Id) -> Self {
        self.id = Some(id);
        self
    }

    #[inline]
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    #[inline]
    pub fn animating(mut self, animating: bool) -> Self {
        self.animating = animating;
        self
    }

    /// Map the message type of the context drawer to another
    #[inline]
    pub fn map<Out: Clone + 'static>(
        self,
        on_message: fn(Message) -> Out,
    ) -> ContextDrawer<'a, Out> {
        ContextDrawer {
            id: self.id,
            content: self.content.map(on_message),
            drawer: self.drawer.map(on_message),
            on_close: self.on_close.map(on_message),
            open: self.open,
            animating: self.animating,
        }
    }

    /// Optionally assigns a message to publish when the user presses outside the drawer to dismiss it.
    #[inline]
    pub fn on_close_maybe(mut self, message: Option<Message>) -> Self {
        self.on_close = message;
        self
    }
}

impl<Message: Clone> Widget<Message, crate::Theme, Renderer> for ContextDrawer<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<DrawerAnimation>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(DrawerAnimation::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content), Tree::new(&self.drawer)]
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(&mut [&mut self.content, &mut self.drawer]);
    }

    fn size(&self) -> iced_core::Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation<()>,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
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
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
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
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
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
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            renderer_style,
            layout,
            cursor,
            viewport,
        );
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        _renderer: &Renderer,
        _viewport: &Rectangle,
        translation: Vector,
    ) -> Option<iced_overlay::Element<'b, Message, crate::Theme, Renderer>> {
        let bounds = layout.bounds();

        let mut position = layout.position();
        position.x += translation.x;
        position.y += translation.y;

        let tree::Tree {
            state, children, ..
        } = tree;
        let animation = state.downcast_mut::<DrawerAnimation>();

        Some(iced_overlay::Element::new(Box::new(Overlay {
            content: &mut self.drawer,
            tree: &mut children[1],
            width: bounds.width,
            position,
            animation,
            open: self.open,
            animating: self.animating,
            on_close: self.on_close.as_ref(),
        })))
    }

    #[cfg(feature = "a11y")]
    /// get the a11y nodes for the widget
    fn a11y_nodes(
        &self,
        layout: Layout<'_>,
        state: &Tree,
        p: mouse::Cursor,
    ) -> iced_accessibility::A11yTree {
        let c_state = &state.children[0];
        self.content.as_widget().a11y_nodes(layout, c_state, p)
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
            layout,
            renderer,
            dnd_rectangles,
        );
    }

    fn id(&self) -> Option<iced_core::widget::Id> {
        self.id.clone()
    }

    fn set_id(&mut self, id: iced_core::widget::Id) {
        self.id = Some(id);
    }
}

impl<'a, Message: 'a + Clone> From<ContextDrawer<'a, Message>> for Element<'a, Message> {
    fn from(widget: ContextDrawer<'a, Message>) -> Element<'a, Message> {
        Element::new(widget)
    }
}

struct Slide<'a, Message> {
    content: Element<'a, Message>,
    width: f32,
    open: bool,
    animating: bool,
}

impl<'a, Message: Clone + 'static> Slide<'a, Message> {
    fn new(
        content: impl Into<Element<'a, Message>>,
        width: f32,
        open: bool,
        animating: bool,
    ) -> Self {
        Self {
            content: content.into(),
            width,
            open,
            animating,
        }
    }
}

impl<Message: Clone> Widget<Message, crate::Theme, Renderer> for Slide<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<DrawerAnimation>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(DrawerAnimation::default())
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
            .downcast_mut::<DrawerAnimation>()
            .progress(self.open, self.animating);

        let max_height = limits.max().height;
        let max_width = limits.max().width;
        let width = self.width.min(max_width);
        let limits =
            layout::Limits::new(iced_core::Size::ZERO, iced_core::Size::new(width, max_height));
        let content = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, &limits);
        let content_size = content.size();

        let visible_width = width * progress;

        layout::Node::with_children(
            iced_core::Size::new(visible_width, content_size.height),
            vec![content],
        )
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
            let animation = tree.state.downcast_mut::<DrawerAnimation>();

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
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout.child(0), renderer, operation);
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
    fn finish(animation: &mut DrawerAnimation) {
        animation.animation.last_change = Some(Instant::now() - ANIMATION_DURATION * 2);
    }

    #[test]
    fn drawer_animation_opens_and_closes() {
        let mut animation = DrawerAnimation::default();

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
    fn drawer_animation_reverses_midway() {
        let mut animation = DrawerAnimation::default();
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
    fn drawer_animation_stays_put_without_animation() {
        let mut animation = DrawerAnimation::default();

        // Opening without animating jumps to the open position.
        assert_eq!(animation.progress(true, false), 1.0);
        assert!(!animation.active());

        // Closing without animating jumps to the hidden position.
        assert_eq!(animation.progress(false, false), 0.0);
        assert!(!animation.active());
    }
}
