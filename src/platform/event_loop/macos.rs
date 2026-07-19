use winit::event_loop::EventLoopBuilder;
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};

pub fn configure<T: 'static>(builder: &mut EventLoopBuilder<T>) {
    // Menu-bar agent: avoid Regular activation (Dock + app menu bar),
    // which dismisses status-item submenus on hover.
    builder
        .with_activation_policy(ActivationPolicy::Accessory)
        .with_default_menu(false)
        .with_activate_ignoring_other_apps(false);
}
