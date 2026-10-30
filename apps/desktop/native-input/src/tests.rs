use super::*;
use zeroize::Zeroizing;

macro_rules! assert_not_impl {
    ($owner:ty: $trait:path) => {
        const _: fn() = || {
            trait AmbiguousIfImpl<Marker> {
                fn marker() {}
            }
            impl<T: ?Sized> AmbiguousIfImpl<()> for T {}
            struct Invalid;
            impl<T: ?Sized + $trait> AmbiguousIfImpl<Invalid> for T {}
            let _ = <$owner as AmbiguousIfImpl<_>>::marker;
        };
    };
}

assert_not_impl!(SecretInput: Clone);
assert_not_impl!(SecretInput: std::fmt::Debug);

fn review_display() -> ReviewDisplay {
    ReviewDisplay {
        origin: "https://openshell.example.test".into(),
        selected_peer: "peer-01".into(),
        installation: "example-installation".into(),
        workspace: "example-workspace".into(),
        sandbox: "example-sandbox".into(),
        product_version: "1.2.3".into(),
        root_sha256: "a".repeat(64),
        peer_sha256: "B".repeat(64),
        method: ReadMethod::ProtectedCheck,
    }
}

#[test]
fn review_display_accepts_exact_bounded_metadata_and_formats_it_readably() {
    let display = review_display();
    assert_eq!(display.validate(), Ok(()));
    assert_eq!(
        display.text(),
        "OpenShell 1.2.3\nTarget: https://openshell.example.test\nSelected peer: peer-01\nInstallation: example-installation\nWorkspace: example-workspace\nSandbox: example-sandbox\nRead: Protected read check\nRoot SHA-256: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\nPeer SHA-256: BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB\n\nOne attempt, within the original 120-second review deadline. Result stays in this session. No action execution or approval is granted. Credentials are entered in the next native step and are not saved."
    );
}

#[test]
fn review_display_rejects_empty_overbound_and_control_metadata() {
    let mut display = review_display();
    display.origin.clear();
    assert_eq!(display.validate(), Err(InputError::InvalidInput));

    let mut display = review_display();
    display.origin = "a".repeat(513);
    assert_eq!(display.validate(), Err(InputError::InvalidInput));

    let updates: [fn(&mut ReviewDisplay); 6] = [
        |display: &mut ReviewDisplay| display.selected_peer = "a".repeat(65),
        |display: &mut ReviewDisplay| display.installation = "a".repeat(129),
        |display: &mut ReviewDisplay| display.workspace = "a".repeat(129),
        |display: &mut ReviewDisplay| display.sandbox = "a".repeat(129),
        |display: &mut ReviewDisplay| display.product_version = "a".repeat(33),
        |display: &mut ReviewDisplay| display.workspace = "bad\nworkspace".into(),
    ];
    for update in updates {
        let mut display = review_display();
        update(&mut display);
        assert_eq!(display.validate(), Err(InputError::InvalidInput));
    }
}

#[test]
fn review_display_requires_exact_hex_pins_and_method_labels() {
    let mut display = review_display();
    display.root_sha256 = "C".repeat(63);
    assert_eq!(display.validate(), Err(InputError::InvalidInput));

    let mut display = review_display();
    display.peer_sha256 = "c".repeat(65);
    assert_eq!(display.validate(), Err(InputError::InvalidInput));

    let mut display = review_display();
    display.root_sha256.replace_range(63.., "g");
    assert_eq!(display.validate(), Err(InputError::InvalidInput));

    assert_eq!(ReadMethod::ProtectedCheck.label(), "Protected read check");
    assert_eq!(
        ReadMethod::PolicyStatus.label(),
        "Policy status and revision metadata"
    );
    assert_eq!(ReadMethod::RecentLogs.label(), "Recent redacted log window");
    assert_eq!(
        ReadMethod::RecentWatch.label(),
        "Bounded recent watch window (up to 30 seconds)"
    );
}

#[test]
fn secret_input_rejects_invalid_values_without_trimming_accepted_values() {
    let retained = SecretInput::new(Zeroizing::new(" token ".into())).unwrap();
    assert_eq!(retained.into_value().as_str(), " token ");

    let maximum = "a".repeat(MAX_SECRET_BYTES);
    assert_eq!(
        SecretInput::new(Zeroizing::new(maximum.clone()))
            .unwrap()
            .into_value()
            .as_str(),
        maximum
    );

    for value in [
        "".into(),
        "a".repeat(MAX_SECRET_BYTES + 1),
        "non-ascii-ñ".into(),
        "line\nbreak".into(),
        "tab\tvalue".into(),
        "delete\u{7f}value".into(),
    ] {
        assert!(matches!(
            SecretInput::new(Zeroizing::new(value)),
            Err(InputError::InvalidInput)
        ));
    }
}

#[test]
fn input_errors_are_fixed_distinct_and_nonsecret() {
    let diagnostics = [
        (
            InputError::Unavailable,
            "native_input_unavailable",
            "Native access entry is unavailable.",
            "Keep working locally; use a qualified native backend for access entry.",
        ),
        (
            InputError::Busy,
            "native_input_busy",
            "Another native dialog is still open.",
            "Finish or cancel the existing dialog before reviewing again.",
        ),
        (
            InputError::Cancelled,
            "native_input_cancelled",
            "Access entry was stopped locally.",
            "Start a new explicit access review if another read is needed.",
        ),
        (
            InputError::Expired,
            "native_input_expired",
            "This access review expired.",
            "Start a new explicit access review if another read is needed.",
        ),
        (
            InputError::InvalidInput,
            "native_input_invalid",
            "Enter one nonempty header-compatible line, up to 16 KiB.",
            "Start a new explicit access review if another read is needed.",
        ),
        (
            InputError::ParentClosed,
            "native_parent_closed",
            "The initiating window closed.",
            "Reopen the task and start a new access review.",
        ),
    ];
    for (error, code, message, recovery) in diagnostics {
        assert_eq!(error.code(), code);
        assert_eq!(error.message(), message);
        assert_eq!(error.recovery(), recovery);
        assert!(!error.code().contains("token"));
        assert!(!error.message().contains("token"));
        assert!(!error.recovery().contains("token"));
    }
    for (index, (error, ..)) in diagnostics.iter().enumerate() {
        assert!(
            !diagnostics[..index]
                .iter()
                .any(|(previous, ..)| previous.code() == error.code())
        );
    }
}
