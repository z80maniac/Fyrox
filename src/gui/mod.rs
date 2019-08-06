pub mod draw;

use crate::{
    utils::{
        pool::{Pool, Handle},
        rcpool::RcHandle,
    },
    math::{
        vec2::Vec2,
        Rect,
    },
    gui::draw::{Color, DrawingContext, FormattedText, CommandKind, FormattedTextBuilder},
    resource::{
        Resource,
        ttf::Font,
    },
};
use glutin::{VirtualKeyCode, MouseButton, WindowEvent, ElementState};
use serde::export::PhantomData;
use std::any::Any;

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum HorizontalAlignment {
    Stretch,
    Left,
    Center,
    Right,
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum VerticalAlignment {
    Stretch,
    Top,
    Center,
    Bottom,
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Thickness {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

impl Thickness {
    pub fn zero() -> Thickness {
        Thickness {
            left: 0.0,
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Visibility {
    Visible,
    Collapsed,
    Hidden,
}

#[derive(Debug)]
pub struct Text {
    need_update: bool,
    text: String,
    font: Handle<Font>,
    vertical_alignment: VerticalAlignment,
    horizontal_alignment: HorizontalAlignment,
    formatted_text: Option<FormattedText>,
}

impl Text {
    pub fn new(text: &str) -> Text {
        Text {
            text: String::from(text),
            need_update: true,
            vertical_alignment: VerticalAlignment::Top,
            horizontal_alignment: HorizontalAlignment::Left,
            formatted_text: Some(FormattedTextBuilder::new().build()),
            font: Handle::none(),
        }
    }

    pub fn set_text(&mut self, text: &str) {
        self.text.clear();
        self.text += text;
        self.need_update = true;
    }

    pub fn get_text(&self) -> &str {
        self.text.as_str()
    }

    pub fn set_font(&mut self, font: Handle<Font>) {
        self.font = font;
        self.need_update = true;
    }

    pub fn set_vertical_alignment(&mut self, valign: VerticalAlignment) {
        self.vertical_alignment = valign;
    }

    pub fn set_horizontal_alignment(&mut self, halign: HorizontalAlignment) {
        self.horizontal_alignment = halign;
    }
}

#[derive(Debug)]
pub struct Border {
    stroke_thickness: Thickness,
    stroke_color: Color,
}

pub struct Image {
    texture: RcHandle<Resource>
}

pub type ButtonClickEventHandler = dyn FnMut(&mut UserInterface, Handle<UINode>);

pub struct Button {
    click: Option<Box<ButtonClickEventHandler>>,
    was_pressed: bool,
}

impl Button {
    pub fn new() -> Button {
        Button {
            click: None,
            was_pressed: false,
        }
    }

    pub fn set_on_click(&mut self, handler: Box<ButtonClickEventHandler>) {
        self.click = Some(handler);
    }
}

pub enum UINodeKind {
    Base,
    /// TODO
    Text(Text),
    /// TODO
    Border(Border),
    /// TODO
    Window,
    /// TODO
    Button(Button),
    /// TODO
    ScrollBar,
    /// TODO
    ScrollViewer,
    /// TODO
    TextBox,
    /// TODO
    Image,
    /// TODO Automatically arranges children by rows and columns
    Grid,
    /// TODO Allows user to directly set position and size of a node
    Canvas,
    /// TODO Allows user to scroll content
    ScrollContentPresenter,
    /// TODO
    SlideSelector,
    /// TODO
    CheckBox,
    UserControl(Box<dyn Any>),
}

#[derive(Copy, Clone, PartialEq)]
pub enum RoutedEventHandlerType {
    MouseMove,
    MouseEnter,
    MouseLeave,
    MouseDown,
    MouseUp,
    Count,
}

pub type EventHandler = dyn FnMut(&mut UserInterface, Handle<UINode>, &mut RoutedEvent);

pub struct UINode {
    kind: UINodeKind,
    /// Desired position relative to parent node
    desired_local_position: Vec2,
    /// Explicit width for node or automatic if NaN (means value is undefined). Default is NaN
    width: f32,
    /// Explicit height for node or automatic if NaN (means value is undefined). Default is NaN
    height: f32,
    /// Screen position of the node
    screen_position: Vec2,
    /// Desired size of the node after Measure pass.
    desired_size: Vec2,
    /// Actual node local position after Arrange pass.
    actual_local_position: Vec2,
    /// Actual size of the node after Arrange pass.
    actual_size: Vec2,
    /// Minimum width and height
    min_size: Vec2,
    /// Maximum width and height
    max_size: Vec2,
    /// Overlay color of the node
    color: Color,
    /// Index of row to which this node belongs
    row: usize,
    /// Index of column to which this node belongs
    column: usize,
    /// Vertical alignment
    vertical_alignment: VerticalAlignment,
    /// Horizontal alignment
    horizontal_alignment: HorizontalAlignment,
    /// Margin (four sides)
    margin: Thickness,
    /// Current visibility state
    visibility: Visibility,
    children: Vec<Handle<UINode>>,
    parent: Handle<UINode>,
    /// Indices of commands in command buffer emitted by the node.
    command_indices: Vec<usize>,
    is_mouse_over: bool,
    event_handlers: [Option<Box<EventHandler>>; RoutedEventHandlerType::Count as usize],
}

pub enum RoutedEventKind {
    MouseDown {
        pos: Vec2,
        button: MouseButton,
    },
    MouseMove {
        pos: Vec2
    },
    MouseUp {
        pos: Vec2,
        button: MouseButton,
    },
    Text {
        symbol: char
    },
    KeyDown {
        code: VirtualKeyCode
    },
    KeyUp {
        code: VirtualKeyCode
    },
    MouseWheel {
        pos: Vec2,
        amount: u32,
    },
    MouseLeave,
    MouseEnter,
}

pub struct RoutedEvent {
    kind: RoutedEventKind,
    handled: bool,
}

impl RoutedEvent {
    pub fn new(kind: RoutedEventKind) -> RoutedEvent {
        RoutedEvent {
            kind,
            handled: false,
        }
    }
}

pub struct UserInterface {
    nodes: Pool<UINode>,
    drawing_context: DrawingContext,
    default_font: Handle<Font>,
    visual_debug: bool,
    /// Every UI node will live on the window-sized canvas.
    root_canvas: Handle<UINode>,
    picked_node: Handle<UINode>,
    prev_picked_node: Handle<UINode>,
    captured_node: Handle<UINode>,
}

#[inline]
fn maxf(a: f32, b: f32) -> f32 {
    if a > b { a } else { b }
}

#[inline]
fn minf(a: f32, b: f32) -> f32 {
    if a < b { a } else { b }
}

struct UnsafeCollectionView<T> {
    items: *const T,
    len: usize,
}

impl<T> UnsafeCollectionView<T> {
    fn empty() -> UnsafeCollectionView<T> {
        UnsafeCollectionView {
            items: std::ptr::null(),
            len: 0,
        }
    }

    fn from_vec(vec: &Vec<T>) -> UnsafeCollectionView<T> {
        UnsafeCollectionView {
            items: vec.as_ptr(),
            len: vec.len(),
        }
    }

    fn iter(&self) -> CollectionViewIterator<T> {
        unsafe {
            CollectionViewIterator {
                current: self.items,
                end: self.items.offset(self.len as isize),
                marker: PhantomData,
            }
        }
    }
}

struct CollectionViewIterator<'a, T> {
    current: *const T,
    end: *const T,
    marker: PhantomData<&'a T>,
}

impl<'a, T> Iterator for CollectionViewIterator<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        unsafe {
            if self.current != self.end {
                let value = self.current;
                self.current = self.current.offset(1);
                Some(&*value)
            } else {
                None
            }
        }
    }
}

impl UserInterface {
    pub fn new(default_font: Handle<Font>) -> UserInterface {
        let mut nodes = Pool::new();
        UserInterface {
            visual_debug: false,
            default_font,
            captured_node: Handle::none(),
            root_canvas: nodes.spawn(UINode::new(UINodeKind::Canvas)),
            nodes,
            drawing_context: DrawingContext::new(),
            picked_node: Handle::none(),
            prev_picked_node: Handle::none(),
        }
    }

    pub fn add_node(&mut self, node: UINode) -> Handle<UINode> {
        let node_handle = self.nodes.spawn(node);
        self.link_nodes(&node_handle, &self.root_canvas.clone());
        node_handle
    }

    pub fn create_button(&mut self, text: &str) -> Handle<UINode> {
        let normal_color = Color::opaque(120, 120, 120);
        let pressed_color = Color::opaque(100, 100, 100);
        let hover_color = Color::opaque(160, 160, 160);
        let mut button_node = UINode::new(UINodeKind::Button(Button::new()));
        button_node.set_width(200.0);
        button_node.set_height(50.0);
        button_node.set_handler(RoutedEventHandlerType::MouseDown, Box::new(move |ui, handle, _evt| {
            ui.capture_mouse(&handle);
            if let Some(button_node) = ui.nodes.borrow_mut(&handle) {
                if let UINodeKind::Button(button) = button_node.get_kind_mut() {
                    button.was_pressed = true;
                }
            }
        }));
        button_node.set_handler(RoutedEventHandlerType::MouseUp, Box::new(move |ui, handle, evt| {
            // Take-Call-PutBack trick to bypass borrow checker
            let mut click_handler = None;

            if let Some(button_node) = ui.nodes.borrow_mut(&handle) {
                if let UINodeKind::Button(button) = button_node.get_kind_mut() {
                    click_handler = button.click.take();
                    button.was_pressed = false;
                }
            }

            if let Some(ref mut handler) = click_handler {
                handler(ui, handle.clone());
                evt.handled = true;
            }

            // Second check required because event handler can remove node.
            if let Some(button_node) = ui.nodes.borrow_mut(&handle) {
                if let UINodeKind::Button(button) = button_node.get_kind_mut() {
                    button.click = click_handler;
                }
            }

            ui.release_mouse_capture();
        }));
        let button_handle = self.add_node(button_node);
        let border = Border { stroke_color: Color::opaque(200, 200, 200), stroke_thickness: Thickness { left: 2.0, right: 2.0, top: 2.0, bottom: 2.0 } };
        let mut text = Text::new(text);
        text.set_font(self.default_font.clone());
        text.set_horizontal_alignment(HorizontalAlignment::Center);
        text.set_vertical_alignment(VerticalAlignment::Center);
        let mut back = UINode::new(UINodeKind::Border(border));
        back.set_handler(RoutedEventHandlerType::MouseEnter, Box::new(move |ui, handle, _evt| {
            if let Some(back) = ui.nodes.borrow_mut(&handle) {
                back.color = hover_color;
            }
        }));
        back.set_handler(RoutedEventHandlerType::MouseLeave, Box::new(move |ui, handle, _evt| {
            if let Some(back) = ui.nodes.borrow_mut(&handle) {
                back.color = normal_color;
            }
        }));
        back.set_handler(RoutedEventHandlerType::MouseDown, Box::new(move |ui, handle, _evt| {
            if let Some(back) = ui.nodes.borrow_mut(&handle) {
                back.color = pressed_color;
            }
        }));
        back.set_handler(RoutedEventHandlerType::MouseUp, Box::new(move |ui, handle, _evt| {
            if let Some(back) = ui.nodes.borrow_mut(&handle) {
                if back.is_mouse_over {
                    back.color = hover_color;
                } else {
                    back.color = normal_color;
                }
            }
        }));
        back.color = normal_color;
        let back_handle = self.add_node(back);
        let text_handle = self.add_node(UINode::new(UINodeKind::Text(text)));
        self.link_nodes(&text_handle, &back_handle);
        self.link_nodes(&back_handle, &button_handle);
        button_handle
    }

    pub fn capture_mouse(&mut self, node: &Handle<UINode>) -> bool {
        if self.captured_node.is_none() {
            if self.nodes.is_valid_handle(node) {
                self.captured_node = node.clone();
                return true;
            }
        }

        false
    }

    pub fn release_mouse_capture(&mut self) {
        self.captured_node = Handle::none();
    }

    /// Links specified child with specified parent.
    #[inline]
    pub fn link_nodes(&mut self, child_handle: &Handle<UINode>, parent_handle: &Handle<UINode>) {
        self.unlink_node(child_handle);
        if let Some(child) = self.nodes.borrow_mut(child_handle) {
            child.parent = parent_handle.clone();
            if let Some(parent) = self.nodes.borrow_mut(parent_handle) {
                parent.children.push(child_handle.clone());
            }
        }
    }

    /// Unlinks specified node from its parent, so node will become root.
    #[inline]
    pub fn unlink_node(&mut self, node_handle: &Handle<UINode>) {
        let mut parent_handle: Handle<UINode> = Handle::none();
        // Replace parent handle of child
        if let Some(node) = self.nodes.borrow_mut(node_handle) {
            parent_handle = node.parent.clone();
            node.parent = Handle::none();
        }
        // Remove child from parent's children list
        if let Some(parent) = self.nodes.borrow_mut(&parent_handle) {
            if let Some(i) = parent.children.iter().position(|h| h == node_handle) {
                parent.children.remove(i);
            }
        }
    }

    #[inline]
    pub fn get_node(&self, node_handle: &Handle<UINode>) -> Option<&UINode> {
        self.nodes.borrow(node_handle)
    }

    #[inline]
    pub fn get_node_mut(&mut self, node_handle: &Handle<UINode>) -> Option<&mut UINode> {
        self.nodes.borrow_mut(node_handle)
    }

    #[inline]
    pub fn get_drawing_context(&self) -> &DrawingContext {
        &self.drawing_context
    }

    #[inline]
    pub fn get_drawing_context_mut(&mut self) -> &mut DrawingContext {
        &mut self.drawing_context
    }

    /// Performs recursive kind-specific measurement of children nodes
    ///
    /// Returns desired size.
    fn measure_override(&mut self, node_kind: &UINodeKind, children: &UnsafeCollectionView<Handle<UINode>>, available_size: &Vec2) -> Vec2 {
        match node_kind {
            // TODO: Type-specific measure
            UINodeKind::Border(border) => {
                let margin_x = border.stroke_thickness.left + border.stroke_thickness.right;
                let margin_y = border.stroke_thickness.top + border.stroke_thickness.bottom;

                let size_for_child = Vec2::make(
                    available_size.x - margin_x,
                    available_size.y - margin_y,
                );
                let mut desired_size = Vec2::new();
                for child_handle in children.iter() {
                    self.measure(child_handle, &size_for_child);

                    if let Some(child) = self.nodes.borrow(child_handle) {
                        if child.desired_size.x > desired_size.x {
                            desired_size.x = child.desired_size.x;
                        }
                        if child.desired_size.y > desired_size.y {
                            desired_size.y = child.desired_size.y;
                        }
                    }
                }
                desired_size.x += margin_x;
                desired_size.y += margin_y;

                desired_size
            }
            UINodeKind::Canvas => {
                let size_for_child = Vec2::make(
                    std::f32::INFINITY,
                    std::f32::INFINITY,
                );

                for child_handle in children.iter() {
                    self.measure(child_handle, &size_for_child);
                }

                Vec2::new()
            }
            // Default measure
            _ => {
                let mut size = Vec2::new();

                for child_handle in children.iter() {
                    self.measure(child_handle, &available_size);

                    if let Some(child) = self.nodes.borrow(child_handle) {
                        if child.desired_size.x > size.x {
                            size.x = child.desired_size.x;
                        }
                        if child.desired_size.y > size.y {
                            size.y = child.desired_size.y;
                        }
                    }
                }

                size
            }
        }
    }

    fn measure(&mut self, node_handle: &Handle<UINode>, available_size: &Vec2) {
        let mut children: UnsafeCollectionView<Handle<UINode>> = UnsafeCollectionView::empty();
        let mut node_kind: *const UINodeKind = std::ptr::null();
        let size_for_child;
        let margin;

        match self.nodes.borrow_mut(&node_handle) {
            None => return,
            Some(node) => {
                margin = Vec2 {
                    x: node.margin.left + node.margin.right,
                    y: node.margin.top + node.margin.bottom,
                };

                size_for_child = Vec2 {
                    x: {
                        let w = if node.width > 0.0 {
                            node.width
                        } else {
                            maxf(0.0, available_size.x - margin.x)
                        };

                        if w > node.max_size.x {
                            node.max_size.x
                        } else if w < node.min_size.x {
                            node.min_size.x
                        } else {
                            w
                        }
                    },
                    y: {
                        let h = if node.height > 0.0 {
                            node.height
                        } else {
                            maxf(0.0, available_size.y - margin.y)
                        };

                        if h > node.max_size.y {
                            node.max_size.y
                        } else if h < node.min_size.y {
                            node.min_size.y
                        } else {
                            h
                        }
                    },
                };

                if node.visibility == Visibility::Visible {
                    // Remember immutable pointer to collection of children nodes on which we'll continue
                    // measure. It is one hundred percent safe to have immutable pointer to list of
                    // children handles, because we guarantee that children collection won't be modified
                    // during measure pass. Also this step *cannot* be performed in parallel so we don't
                    // have to bother about thread-safety here.
                    children = UnsafeCollectionView::from_vec(&node.children);
                    node_kind = &node.kind as *const UINodeKind;
                } else {
                    // We do not have any children so node want to collapse into point.
                    node.desired_size = Vec2::make(0.0, 0.0);
                }
            }
        }

        let desired_size = self.measure_override(unsafe { &*node_kind }, &children, &size_for_child);

        if let Some(node) = self.nodes.borrow_mut(&node_handle) {
            node.desired_size = desired_size;

            if !node.width.is_nan() {
                node.desired_size.x = node.width;
            }

            if node.desired_size.x > node.max_size.x {
                node.desired_size.x = node.max_size.x;
            } else if node.desired_size.x < node.min_size.x {
                node.desired_size.x = node.min_size.x;
            }

            if node.desired_size.y > node.max_size.y {
                node.desired_size.y = node.max_size.y;
            } else if node.desired_size.y < node.min_size.y {
                node.desired_size.y = node.min_size.y;
            }

            if !node.height.is_nan() {
                node.desired_size.y = node.height;
            }

            node.desired_size += margin;

            // Make sure that node won't go outside of available bounds.
            if node.desired_size.x > available_size.x {
                node.desired_size.x = available_size.x;
            }
            if node.desired_size.y > available_size.y {
                node.desired_size.y = available_size.y;
            }
        }
    }

    /// Performs recursive kind-specific arrangement of children nodes
    ///
    /// Returns actual size.
    fn arrange_override(&mut self, node_kind: &UINodeKind, children: &UnsafeCollectionView<Handle<UINode>>, final_size: &Vec2) -> Vec2 {
        match node_kind {
            // TODO: Kind-specific arrangement
            UINodeKind::Border(border) => {
                let rect_for_child = Rect::new(
                    border.stroke_thickness.left, border.stroke_thickness.top,
                    final_size.x - (border.stroke_thickness.right + border.stroke_thickness.left),
                    final_size.y - (border.stroke_thickness.bottom + border.stroke_thickness.top),
                );

                for child_handle in children.iter() {
                    self.arrange(child_handle, &rect_for_child);
                }

                *final_size
            }
            UINodeKind::Canvas => {
                for child_handle in children.iter() {
                    let mut final_rect = None;

                    if let Some(child) = self.nodes.borrow(&child_handle) {
                        final_rect = Some(Rect::new(
                            child.desired_local_position.x,
                            child.desired_local_position.y,
                            child.desired_size.x,
                            child.desired_size.y));
                    }

                    if let Some(rect) = final_rect {
                        self.arrange(child_handle, &rect);
                    }
                }

                *final_size
            }
            // Default arrangement
            _ => {
                let final_rect = Rect::new(0.0, 0.0, final_size.x, final_size.y);

                for child_handle in children.iter() {
                    self.arrange(child_handle, &final_rect);
                }

                *final_size
            }
        }
    }

    fn arrange(&mut self, node_handle: &Handle<UINode>, final_rect: &Rect<f32>) {
        let children: UnsafeCollectionView<Handle<UINode>>;

        let mut size;
        let size_without_margin;
        let mut origin_x;
        let mut origin_y;
        let node_kind: *const UINodeKind;

        match self.nodes.borrow_mut(node_handle) {
            None => return,
            Some(node) => {
                if node.visibility != Visibility::Visible {
                    return;
                }

                let margin_x = node.margin.left + node.margin.right;
                let margin_y = node.margin.top + node.margin.bottom;

                origin_x = final_rect.x + node.margin.left;
                origin_y = final_rect.y + node.margin.top;

                size = Vec2 {
                    x: maxf(0.0, final_rect.w - margin_x),
                    y: maxf(0.0, final_rect.h - margin_y),
                };

                size_without_margin = size;

                if node.horizontal_alignment != HorizontalAlignment::Stretch {
                    size.x = minf(size.x, node.desired_size.x - margin_x);
                }
                if node.vertical_alignment != VerticalAlignment::Stretch {
                    size.y = minf(size.y, node.desired_size.y - margin_y);
                }

                if node.width > 0.0 {
                    size.x = node.width;
                }
                if node.height > 0.0 {
                    size.y = node.height;
                }

                // Remember immutable pointer to collection of children nodes on which
                // we'll continue arrange recursively.
                children = UnsafeCollectionView::from_vec(&node.children);
                node_kind = &node.kind as *const UINodeKind;
            }
        }

        size = self.arrange_override(unsafe { &*node_kind }, &children, &size);

        if let Some(node) = self.nodes.borrow_mut(node_handle) {
            if size.x > final_rect.w {
                size.x = final_rect.w;
            }
            if size.y > final_rect.h {
                size.y = final_rect.h;
            }

            match node.horizontal_alignment {
                HorizontalAlignment::Center | HorizontalAlignment::Stretch => {
                    origin_x += (size_without_margin.x - size.x) * 0.5;
                }
                HorizontalAlignment::Right => {
                    origin_x += size_without_margin.x - size.x;
                }
                _ => ()
            }

            match node.vertical_alignment {
                VerticalAlignment::Center | VerticalAlignment::Stretch => {
                    origin_y += (size_without_margin.y - size.y) * 0.5;
                }
                VerticalAlignment::Bottom => {
                    origin_y += size_without_margin.y - size.y;
                }
                _ => ()
            }

            node.actual_size = size;
            node.actual_local_position = Vec2 { x: origin_x, y: origin_y };
        }
    }

    fn update_transform(&mut self, node_handle: &Handle<UINode>) {
        let mut children = UnsafeCollectionView::empty();

        let mut screen_position = Vec2::new();
        if let Some(node) = self.nodes.borrow(node_handle) {
            children = UnsafeCollectionView::from_vec(&node.children);
            if let Some(parent) = self.nodes.borrow(&node.parent) {
                screen_position = node.actual_local_position + parent.screen_position;
            } else {
                screen_position = node.actual_local_position;
            }
        }

        if let Some(node) = self.nodes.borrow_mut(node_handle) {
            node.screen_position = screen_position;
        }

        // Continue on children
        for child_handle in children.iter() {
            self.update_transform(child_handle);
        }
    }

    pub fn update(&mut self, screen_size: &Vec2) {
        let root_canvas_handle = self.root_canvas.clone();
        self.measure(&root_canvas_handle, screen_size);
        self.arrange(&root_canvas_handle, &Rect::new(0.0, 0.0, screen_size.x, screen_size.y));
        self.update_transform(&root_canvas_handle);
    }

    fn draw_node(&mut self, node_handle: &Handle<UINode>, font_cache: &Pool<Font>, nesting: u8) {
        let mut children: UnsafeCollectionView<Handle<UINode>> = UnsafeCollectionView::empty();

        if let Some(node) = self.nodes.borrow_mut(node_handle) {
            let bounds = node.get_screen_bounds();

            self.drawing_context.set_nesting(nesting);
            node.command_indices.push(self.drawing_context.commit_clip_rect(&bounds.inflate(0.9, 0.9)));


            match &mut node.kind {
                UINodeKind::Border(border) => {
                    self.drawing_context.push_rect_filled(&bounds, None, node.color);
                    self.drawing_context.push_rect_vary(&bounds, border.stroke_thickness, border.stroke_color);
                    node.command_indices.push(self.drawing_context.commit(CommandKind::Geometry, 0).unwrap());
                }
                UINodeKind::Text(text) => {
                    if text.need_update {
                        if let Some(font) = font_cache.borrow(&text.font) {
                            let formatted_text = FormattedTextBuilder::reuse(text.formatted_text.take().unwrap())
                                .with_size(node.actual_size)
                                .with_font(font)
                                .with_text(text.text.as_str())
                                .with_color(node.color)
                                .with_horizontal_alignment(text.horizontal_alignment)
                                .with_vertical_alignment(text.vertical_alignment)
                                .build();
                            text.formatted_text = Some(formatted_text);
                        }
                        text.need_update = true; // TODO
                    }
                    if let Some(command_index) = self.drawing_context.draw_text(node.screen_position, text.formatted_text.as_ref().unwrap()) {
                        node.command_indices.push(command_index);
                    }
                }
                _ => ()
            }

            children = UnsafeCollectionView::from_vec(&node.children);
        }

        // Continue on children
        for child_node in children.iter() {
            self.draw_node(child_node, font_cache, nesting + 1);
        }

        self.drawing_context.revert_clip_geom();
    }

    pub fn draw(&mut self, font_cache: &Pool<Font>) -> &DrawingContext {
        self.drawing_context.clear();

        let root_canvas = self.root_canvas.clone();
        self.draw_node(&root_canvas, font_cache, 1);

        if self.visual_debug {
            self.drawing_context.set_nesting(0);

            let picked_bounds =
                if let Some(picked_node) = self.nodes.borrow(&self.picked_node) {
                    Some(picked_node.get_screen_bounds())
                } else {
                    None
                };

            if let Some(picked_bounds) = picked_bounds {
                self.drawing_context.push_rect(&picked_bounds, 1.0, Color::white());
                self.drawing_context.commit(CommandKind::Geometry, 0);
            }
        }

        &self.drawing_context
    }

    fn is_node_clipped(&self, node_handle: &Handle<UINode>, pt: &Vec2) -> bool {
        let mut clipped = true;

        if let Some(node) = self.nodes.borrow(node_handle) {
            if node.visibility != Visibility::Visible {
                return clipped;
            }

            for command_index in node.command_indices.iter() {
                if let Some(command) = self.drawing_context.get_commands().get(*command_index) {
                    if *command.get_kind() == CommandKind::Clip {
                        if self.drawing_context.is_command_contains_point(command, pt) {
                            clipped = false;

                            break;
                        }
                    }
                }
            }

            // Point can be clipped by parent's clipping geometry.
            if !node.parent.is_none() {
                if !clipped {
                    clipped |= self.is_node_clipped(&node.parent, pt);
                }
            }
        }

        clipped
    }

    fn is_node_contains_point(&self, node_handle: &Handle<UINode>, pt: &Vec2) -> bool {
        if let Some(node) = self.nodes.borrow(node_handle) {
            if node.visibility != Visibility::Visible {
                return false;
            }

            if !self.is_node_clipped(node_handle, pt) {
                for command_index in node.command_indices.iter() {
                    if let Some(command) = self.drawing_context.get_commands().get(*command_index) {
                        if *command.get_kind() == CommandKind::Geometry {
                            if self.drawing_context.is_command_contains_point(command, pt) {
                                return true;
                            }
                        }
                    }
                }
            }
        }

        false
    }

    fn pick_node(&self, node_handle: &Handle<UINode>, pt: &Vec2, level: &mut i32) -> Handle<UINode> {
        let mut picked = Handle::none();
        let mut topmost_picked_level = 0;

        if self.is_node_contains_point(node_handle, pt) {
            picked = node_handle.clone();
            topmost_picked_level = *level;
        }

        if let Some(node) = self.nodes.borrow(node_handle) {
            for child_handle in node.children.iter() {
                *level += 1;
                let picked_child = self.pick_node(child_handle, pt, level);
                if !picked_child.is_none() && *level > topmost_picked_level {
                    topmost_picked_level = *level;
                    picked = picked_child;
                }
            }
        }

        return picked;
    }

    pub fn hit_test(&self, pt: &Vec2) -> Handle<UINode> {
        let mut level = 0;
        let node =
            if self.nodes.is_valid_handle(&self.captured_node) {
                self.captured_node.clone()
            } else {
                self.root_canvas.clone()
            };
        self.pick_node(&node, pt, &mut level)
    }

    fn route_event(&mut self, node_handle: Handle<UINode>, event_type: RoutedEventHandlerType, event_args: &mut RoutedEvent) {
        let mut handler = None;
        let mut parent = Handle::none();
        let index = event_type as usize;

        if let Some(node) = self.nodes.borrow_mut(&node_handle) {
            // Take event handler.
            handler = node.event_handlers[index].take();
            parent = node.parent.clone();
        }

        // Execute event handler.
        if let Some(ref mut mouse_enter) = handler {
            mouse_enter(self, node_handle.clone(), event_args);
        }

        if let Some(node) = self.nodes.borrow_mut(&node_handle) {
            // Put event handler back.
            node.event_handlers[index] = handler.take();
        }

        // Route event up on hierarchy (bubbling strategy) until is not handled.
        if !event_args.handled && !parent.is_none() {
            self.route_event(parent, event_type, event_args);
        }
    }

    pub fn process_event(&mut self, event: &glutin::WindowEvent) -> bool {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                let pos = Vec2::make(position.x as f32, position.y as f32);
                self.picked_node = self.hit_test(&pos);

                // Fire mouse leave for previously picked node
                if self.picked_node != self.prev_picked_node {
                    let mut fire_mouse_leave = false;
                    if let Some(prev_picked_node) = self.nodes.borrow_mut(&self.prev_picked_node) {
                        if prev_picked_node.is_mouse_over {
                            prev_picked_node.is_mouse_over = false;
                            fire_mouse_leave = true;
                        }
                    }

                    if fire_mouse_leave {
                        let mut evt = RoutedEvent::new(RoutedEventKind::MouseLeave);
                        self.route_event(self.prev_picked_node.clone(), RoutedEventHandlerType::MouseLeave, &mut evt);
                    }
                }

                if !self.picked_node.is_none() {
                    let mut fire_mouse_enter = false;
                    if let Some(picked_node) = self.nodes.borrow_mut(&self.picked_node) {
                        if !picked_node.is_mouse_over {
                            picked_node.is_mouse_over = true;
                            fire_mouse_enter = true;
                        }
                    }

                    if fire_mouse_enter {
                        let mut evt = RoutedEvent::new(RoutedEventKind::MouseEnter);
                        self.route_event(self.picked_node.clone(), RoutedEventHandlerType::MouseEnter, &mut evt);
                    }

                    // Fire mouse move
                    let mut evt = RoutedEvent::new(RoutedEventKind::MouseMove { pos });
                    self.route_event(self.picked_node.clone(), RoutedEventHandlerType::MouseMove, &mut evt);
                }
            }
            _ => ()
        }

        if !self.picked_node.is_none() {
            match event {
                WindowEvent::MouseInput { button, state, .. } => {
                    match state {
                        ElementState::Pressed => {
                            let mut evt = RoutedEvent::new(RoutedEventKind::MouseDown {
                                pos: Vec2::new(),
                                button: *button,
                            });
                            self.route_event(self.picked_node.clone(), RoutedEventHandlerType::MouseDown, &mut evt);
                        }
                        ElementState::Released => {
                            let mut evt = RoutedEvent::new(RoutedEventKind::MouseUp {
                                pos: Vec2::new(),
                                button: *button,
                            });
                            self.route_event(self.picked_node.clone(), RoutedEventHandlerType::MouseUp, &mut evt);
                        }
                    }
                }
                _ => ()
            }
        }

        self.prev_picked_node = self.picked_node.clone();

        false
    }
}

impl UINode {
    pub fn new(kind: UINodeKind) -> UINode {
        UINode {
            kind,
            desired_local_position: Vec2::new(),
            width: std::f32::NAN,
            height: std::f32::NAN,
            screen_position: Vec2::new(),
            desired_size: Vec2::new(),
            actual_local_position: Vec2::new(),
            actual_size: Vec2::new(),
            min_size: Vec2::make(0.0, 0.0),
            max_size: Vec2::make(std::f32::INFINITY, std::f32::INFINITY),
            color: Color::white(),
            row: 0,
            column: 0,
            vertical_alignment: VerticalAlignment::Stretch,
            horizontal_alignment: HorizontalAlignment::Stretch,
            margin: Thickness::zero(),
            visibility: Visibility::Visible,
            children: Vec::new(),
            parent: Handle::none(),
            command_indices: Vec::new(),
            event_handlers: Default::default(),
            is_mouse_over: false,
        }
    }

    pub fn set_width(&mut self, width: f32) {
        self.width = width;
    }

    pub fn set_height(&mut self, height: f32) {
        self.height = height;
    }

    pub fn set_desired_local_position(&mut self, pos: Vec2) {
        self.desired_local_position = pos;
    }

    pub fn get_kind(&self) -> &UINodeKind {
        &self.kind
    }

    pub fn set_vertical_alignment(&mut self, valign: VerticalAlignment) {
        self.vertical_alignment = valign;
    }

    pub fn set_horizontal_alignment(&mut self, halign: HorizontalAlignment) {
        self.horizontal_alignment = halign;
    }

    pub fn get_kind_mut(&mut self) -> &mut UINodeKind {
        &mut self.kind
    }

    pub fn get_screen_bounds(&self) -> Rect<f32> {
        Rect::new(self.screen_position.x, self.screen_position.y, self.actual_size.x, self.actual_size.y)
    }

    pub fn set_handler(&mut self, handler_type: RoutedEventHandlerType, handler: Box<EventHandler>) {
        self.event_handlers[handler_type as usize] = Some(handler);
    }
}