//! A secret value that must never leave via `Debug`, `Display`, or `serde`.
//!
//! Mirrors the redaction pattern in `chimera_provider::keychain::SecretRef`
//! (this crate cannot depend on chimera-provider — see lib.rs — so the
//! pattern is duplicated rather than shared). A migrated API key passes
//! through this wrapper from the moment it is read out of a 1.x/CC-Switch
//! file until the instant it is handed to a [`crate::ports::KeychainSink`];
//! it is never stored in a struct that gets logged or serialised.
use std::fmt;

/// A secret string with a `Debug` impl that never prints the value.
///
/// Deliberately has no `Serialize` impl at all: a type that cannot be
/// serialised cannot accidentally end up in a preview payload or a log file.
#[derive(Clone)]
pub struct RedactedSecret(String);

impl RedactedSecret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The only way to get the real value back out. Callers must treat the
    /// result the same way: never `Debug`, never log, never persist as-is.
    pub fn reveal(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for RedactedSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RedactedSecret(<redacted>)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_contains_the_secret_value() {
        let secret = RedactedSecret::new("sk-do-not-print-me");
        let rendered = format!("{secret:?}");
        assert!(!rendered.contains("sk-do-not-print-me"));
        assert_eq!(secret.reveal(), "sk-do-not-print-me");
    }
}
