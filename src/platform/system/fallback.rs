use anyhow::{Context, Result};

use crate::platform::traits::SystemProxy;

/// Try primary first; on failure fall back to secondary.
pub struct FallbackProxy<P, S> {
    primary: P,
    secondary: S,
}

impl<P, S> FallbackProxy<P, S> {
    pub fn new(primary: P, secondary: S) -> Self {
        Self { primary, secondary }
    }
}

impl<P, S> SystemProxy for FallbackProxy<P, S>
where
    P: SystemProxy,
    S: SystemProxy,
{
    fn enable(&self, http_port: u16, socks_port: u16) -> Result<()> {
        match self.primary.enable(http_port, socks_port) {
            Ok(()) => Ok(()),
            Err(e) => self
                .secondary
                .enable(http_port, socks_port)
                .with_context(|| format!("primary failed ({e}); secondary also failed")),
        }
    }

    fn disable(&self) -> Result<()> {
        match self.primary.disable() {
            Ok(()) => Ok(()),
            Err(e) => self
                .secondary
                .disable()
                .with_context(|| format!("primary disable failed ({e}); secondary also failed")),
        }
    }

    fn is_enabled(&self) -> bool {
        self.primary.is_enabled()
    }
}
