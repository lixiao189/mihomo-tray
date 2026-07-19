use winit::event_loop::EventLoopBuilder;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(target_os = "macos"))]
mod default;

/// Injected event-loop presentation policy for the host OS.
#[derive(Debug, Clone, Copy)]
pub struct EventLoopHost {
    backend: Backend,
}

#[derive(Debug, Clone, Copy)]
enum Backend {
    #[cfg(target_os = "macos")]
    Macos,
    #[cfg(not(target_os = "macos"))]
    Default,
}

impl EventLoopHost {
    pub fn for_host() -> Self {
        Self {
            backend: {
                #[cfg(target_os = "macos")]
                {
                    Backend::Macos
                }
                #[cfg(not(target_os = "macos"))]
                {
                    Backend::Default
                }
            },
        }
    }

    pub fn configure<T: 'static>(self, builder: &mut EventLoopBuilder<T>) {
        match self.backend {
            #[cfg(target_os = "macos")]
            Backend::Macos => macos::configure(builder),
            #[cfg(not(target_os = "macos"))]
            Backend::Default => default::configure(builder),
        }
    }
}
