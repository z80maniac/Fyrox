use crate::WindowEvent;
use fyrox::core::algebra::Vector2;
use fyrox::event::Event;
use fyrox::gui::message::MessageDirection;
use fyrox::gui::widget::WidgetMessage;
use fyrox::{
    core::pool::Handle,
    gui::{window::Window, UiNode, UserInterface},
};

pub mod path_fixer;

pub fn is_slice_equal_permutation<T: PartialEq>(a: &[T], b: &[T]) -> bool {
    if a.is_empty() && !b.is_empty() {
        false
    } else {
        // TODO: Find a way to do this faster.
        for source in a.iter() {
            let mut found = false;
            for other in b.iter() {
                if other == source {
                    found = true;
                    break;
                }
            }
            if !found {
                return false;
            }
        }
        true
    }
}

pub fn window_content(window: Handle<UiNode>, ui: &UserInterface) -> Handle<UiNode> {
    ui.node(window)
        .cast::<Window>()
        .map(|w| w.content())
        .unwrap_or_default()
}

pub fn enable_widget(handle: Handle<UiNode>, state: bool, ui: &UserInterface) {
    ui.send_message(WidgetMessage::enabled(
        handle,
        MessageDirection::ToWidget,
        state,
    ));
}

pub fn normalize_os_event(
    result: &mut Event<()>,
    frame_position: Vector2<f32>,
    frame_size: Vector2<f32>,
) {
    if let Event::WindowEvent { event, .. } = result {
        match event {
            WindowEvent::Resized(size) => {
                size.width = frame_size.x as u32;
                size.height = frame_size.y as u32;
            }
            WindowEvent::Moved(position) => {
                position.x -= frame_position.x as i32;
                position.y -= frame_position.y as i32;
            }
            WindowEvent::CursorMoved { position, .. } => {
                position.x -= frame_position.x as f64;
                position.y -= frame_position.y as f64;
            }
            WindowEvent::Touch(touch) => {
                touch.location.x -= frame_position.x as f64;
                touch.location.y -= frame_position.y as f64;
            }
            WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                new_inner_size.width = frame_size.x as u32;
                new_inner_size.height = frame_size.y as u32;
            }
            _ => (),
        }
    }
}
