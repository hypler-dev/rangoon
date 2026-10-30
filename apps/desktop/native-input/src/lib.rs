//! Unmounted native review and secret entry. The caller owns consent scope,
//! generation and admission; this crate owns only native widget lifetimes.
//! Toolkit copies are outside the Rust allocation cleanup guarantee.
#![deny(unsafe_code)]

use std::sync::Arc;
use tauri::{Runtime, Window};
#[cfg(any(test, target_os = "macos"))]
use zeroize::Zeroize;
use zeroize::Zeroizing;

#[cfg(target_os = "macos")]
mod macos;

pub const MAX_SECRET_BYTES: usize = 16 * 1024;

/// Fixed, payload-free diagnostics for humans and structured callers. No
/// secret, target, toolkit exception or provider response enters these fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputError {
    Unavailable,
    Busy,
    Cancelled,
    Expired,
    InvalidInput,
    ParentClosed,
}

impl InputError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Unavailable => "native_input_unavailable",
            Self::Busy => "native_input_busy",
            Self::Cancelled => "native_input_cancelled",
            Self::Expired => "native_input_expired",
            Self::InvalidInput => "native_input_invalid",
            Self::ParentClosed => "native_parent_closed",
        }
    }

    pub const fn message(self) -> &'static str {
        match self {
            Self::Unavailable => "Native access entry is unavailable.",
            Self::Busy => "Another native dialog is still open.",
            Self::Cancelled => "Access entry was stopped locally.",
            Self::Expired => "This access review expired.",
            Self::InvalidInput => "Enter one nonempty header-compatible line, up to 16 KiB.",
            Self::ParentClosed => "The initiating window closed.",
        }
    }

    pub const fn recovery(self) -> &'static str {
        match self {
            Self::Unavailable => {
                "Keep working locally; use a qualified native backend for access entry."
            }
            Self::Busy => "Finish or cancel the existing dialog before reviewing again.",
            Self::Cancelled | Self::Expired | Self::InvalidInput => {
                "Start a new explicit access review if another read is needed."
            }
            Self::ParentClosed => "Reopen the task and start a new access review.",
        }
    }
}

/// The existing host supplies a generation/cancellation/deadline check. This
/// is not a second session registry or permission to acquire a credential.
pub trait InputGuard: Send + Sync + 'static {
    fn check(&self) -> Result<(), InputError>;

    /// Payload-free observation only after native review dispatch succeeds.
    /// This acknowledges queue acceptance, not visible UI or authorization.
    fn review_queued(&self) {}

    /// Payload-free observation after the native sheet and secure field have
    /// been presented. This notification is not consent or authorization.
    fn input_presented(&self) {}
}

/// Only nonsecret, already selected target metadata. Validation happens again
/// before presentation so a caller cannot turn review text into arbitrary UI.
#[derive(Clone)]
pub struct ReviewDisplay {
    pub origin: String,
    pub selected_peer: String,
    pub installation: String,
    pub workspace: String,
    pub sandbox: String,
    pub product_version: String,
    pub root_sha256: String,
    pub peer_sha256: String,
    pub method: ReadMethod,
}

#[derive(Clone, Copy)]
pub enum ReadMethod {
    ProtectedCheck,
    PolicyStatus,
    RecentLogs,
    RecentWatch,
}

#[cfg(any(test, target_os = "macos"))]
impl ReadMethod {
    fn label(self) -> &'static str {
        match self {
            Self::ProtectedCheck => "Protected read check",
            Self::PolicyStatus => "Policy status and revision metadata",
            Self::RecentLogs => "Recent redacted log window",
            Self::RecentWatch => "Bounded recent watch window (up to 30 seconds)",
        }
    }
}

#[cfg(any(test, target_os = "macos"))]
impl ReviewDisplay {
    fn validate(&self) -> Result<(), InputError> {
        for (value, bound) in [
            (&self.origin, 512),
            (&self.selected_peer, 64),
            (&self.installation, 128),
            (&self.workspace, 128),
            (&self.sandbox, 128),
            (&self.product_version, 32),
        ] {
            if value.is_empty()
                || value.len() > bound
                || !value.bytes().all(|b| (0x20..=0x7e).contains(&b))
            {
                return Err(InputError::InvalidInput);
            }
        }
        if [&self.root_sha256, &self.peer_sha256]
            .into_iter()
            .any(|s| s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err(InputError::InvalidInput);
        }
        Ok(())
    }

    fn text(&self) -> String {
        format!(
            "OpenShell {}\nTarget: {}\nSelected peer: {}\nInstallation: {}\nWorkspace: {}\nSandbox: {}\nRead: {}\nRoot SHA-256: {}\nPeer SHA-256: {}\n\nOne attempt, within the original 120-second review deadline. Result stays in this session. No action execution or approval is granted. Credentials are entered in the next native step and are not saved.",
            self.product_version,
            self.origin,
            self.selected_peer,
            self.installation,
            self.workspace,
            self.sandbox,
            self.method.label(),
            self.root_sha256,
            self.peer_sha256
        )
    }
}

/// Move-only Rust cleanup custody. No Debug, Clone, serialization or secret
/// accessor for display. The native control is cleared before this is returned.
pub struct SecretInput(Zeroizing<String>);

impl SecretInput {
    #[cfg(any(test, target_os = "macos"))]
    fn new(mut value: Zeroizing<String>) -> Result<Self, InputError> {
        if value.is_empty()
            || value.len() > MAX_SECRET_BYTES
            || !value.bytes().all(|b| (0x20..=0x7e).contains(&b))
        {
            value.zeroize();
            return Err(InputError::InvalidInput);
        }
        Ok(Self(value))
    }

    pub fn into_value(self) -> Zeroizing<String> {
        self.0
    }
}

/// Blocking worker-side call. Never invoke on the native main thread. Success
/// can only follow the actual native review completion callback.
pub fn confirm<R: Runtime>(
    parent: Window<R>,
    display: ReviewDisplay,
    guard: Arc<dyn InputGuard>,
) -> Result<(), InputError> {
    #[cfg(target_os = "macos")]
    {
        display.validate()?;
        guard.check()?;
        macos::confirm(parent, display.text(), guard)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (parent, display, guard);
        Err(InputError::Unavailable)
    }
}

/// Call only after exact native confirmation under the same retained host
/// flight. This function grants no claim or engine authorization itself.
pub fn acquire<R: Runtime>(
    parent: Window<R>,
    guard: Arc<dyn InputGuard>,
) -> Result<SecretInput, InputError> {
    #[cfg(target_os = "macos")]
    {
        guard.check()?;
        macos::acquire(parent, guard)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (parent, guard);
        Err(InputError::Unavailable)
    }
}

#[cfg(test)]
mod tests;
