//! `Area`: a wrapper widget that adds a hover-aware background, click, double-click
//! and right-click handling, and layout-bounds reporting (used for scroll-into-view
//! and drag-and-drop hit testing). Cards and section headers use it.

use iced::advanced::layout::{self, Layout};
use iced::advanced::mouse;
use iced::advanced::renderer;
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Shell, Widget, overlay};
use iced::widget::container;
use iced::{Color, Element, Event, Length, Point, Rectangle, Size, Theme, Vector};

type Renderer = iced::Renderer;
type StyleFn<'a> = Box<dyn Fn(&Theme, bool) -> container::Style + 'a>;

pub struct Area<'a, Message> {
    content: Element<'a, Message>,
    on_press: Option<Box<dyn Fn(Point) -> Message + 'a>>,
    on_double_click: Option<Message>,
    on_right_press: Option<Message>,
    on_bounds: Option<Box<dyn Fn(Rectangle) -> Message + 'a>>,
    style: Option<StyleFn<'a>>,
    interaction: Option<mouse::Interaction>,
}

pub fn area<'a, Message>(content: impl Into<Element<'a, Message>>) -> Area<'a, Message> {
    Area {
        content: content.into(),
        on_press: None,
        on_double_click: None,
        on_right_press: None,
        on_bounds: None,
        style: None,
        interaction: None,
    }
}

impl<'a, Message> Area<'a, Message> {
    /// Left press; receives the cursor position relative to the area.
    pub fn on_press(mut self, f: impl Fn(Point) -> Message + 'a) -> Self {
        self.on_press = Some(Box::new(f));
        self
    }

    pub fn on_double_click(mut self, m: Message) -> Self {
        self.on_double_click = Some(m);
        self
    }

    pub fn on_right_press(mut self, m: Message) -> Self {
        self.on_right_press = Some(m);
        self
    }

    /// Reports layout bounds whenever they change.
    pub fn on_bounds(mut self, f: impl Fn(Rectangle) -> Message + 'a) -> Self {
        self.on_bounds = Some(Box::new(f));
        self
    }

    pub fn style(mut self, f: impl Fn(&Theme, bool) -> container::Style + 'a) -> Self {
        self.style = Some(Box::new(f));
        self
    }

    pub fn interaction(mut self, i: mouse::Interaction) -> Self {
        self.interaction = Some(i);
        self
    }
}

#[derive(Default)]
struct State {
    hovered: bool,
    bounds: Option<Rectangle>,
    previous_click: Option<mouse::Click>,
}

impl<Message: Clone> Widget<Message, Theme, Renderer> for Area<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(&mut self, tree: &mut Tree, layout: Layout<'_>, renderer: &Renderer, operation: &mut dyn Operation) {
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

        let state = tree.state.downcast_mut::<State>();
        let bounds = layout.bounds();

        if let Some(f) = &self.on_bounds
            && state.bounds != Some(bounds)
        {
            state.bounds = Some(bounds);
            shell.publish(f(bounds));
        }

        let hovered = cursor.is_over(bounds);
        if hovered != state.hovered {
            state.hovered = hovered;
            if self.style.is_some() {
                shell.request_redraw();
            }
        }

        if shell.is_event_captured() {
            return;
        }

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(local) = cursor.position_in(bounds) else {
                    return;
                };
                if let Some(msg) = &self.on_double_click
                    && let Some(pos) = cursor.position()
                {
                    let click = mouse::Click::new(pos, mouse::Button::Left, state.previous_click);
                    state.previous_click = Some(click);
                    if click.kind() == mouse::click::Kind::Double {
                        shell.publish(msg.clone());
                        shell.capture_event();
                        return;
                    }
                }
                if let Some(f) = &self.on_press {
                    shell.publish(f(local));
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                if cursor.is_over(bounds)
                    && let Some(msg) = &self.on_right_press
                {
                    shell.publish(msg.clone());
                    shell.capture_event();
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
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let inner = self
            .content
            .as_widget()
            .mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer);
        match (self.interaction, inner) {
            (Some(i), mouse::Interaction::None) if cursor.is_over(layout.bounds()) => i,
            _ => inner,
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();
        let mut text_style = *style;

        if let Some(f) = &self.style {
            let s = f(theme, state.hovered);
            if bounds.intersects(viewport) {
                use iced::advanced::Renderer as _;
                if s.background.is_some() || s.border.width > 0.0 || s.shadow.color.a > 0.0 {
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds,
                            border: s.border,
                            shadow: s.shadow,
                            snap: s.snap,
                        },
                        s.background.unwrap_or(Color::TRANSPARENT.into()),
                    );
                }
            }
            if let Some(c) = s.text_color {
                text_style.text_color = c;
            }
        }

        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            &text_style,
            layout,
            cursor,
            viewport,
        );
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content
            .as_widget_mut()
            .overlay(&mut tree.children[0], layout, renderer, viewport, translation)
    }
}

impl<'a, Message: Clone + 'a> From<Area<'a, Message>> for Element<'a, Message> {
    fn from(a: Area<'a, Message>) -> Self {
        Element::new(a)
    }
}
