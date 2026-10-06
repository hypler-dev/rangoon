# Native model workbench and consent integration

Authority: the accepted October 6 model-assistance continuation in [intent.md](intent.md). This controller specification refines [model-native-session.md](model-native-session.md) for actual desktop integration. The published session/transport contracts remain unchanged unless explicitly refined below. Evidence and publication state belong in [development.md](development.md). This is implementation direction, not a claim of qualified GUI support.

## Native-owned review surface

A standard OS alert is insufficient for sixteen complete record identities and hashes. The native confirmation surface therefore consists of an isolated, native-created `model-confirmation` WebView window and a final OS message dialog parented to that review window. The review window is parented to `main`, loads only the bundled `model-confirmation.html` path, rejects other navigation, and receives a separate capability allowlist. This replaces the earlier assumption that all selected-record metadata fits directly into one OS alert parented to `main`.

The review window displays native-retained origin, model, selected references/digests/observed heads/byte counts, pack/request IDs and exact outgoing body. Its fixed script inserts dynamic values only through `textContent`; it reads no storage, URL payload, opener, events, shared application modules or renderer-authored approval value. Its scrollable list preserves all sixteen entries and full hashes. It offers `Send selected text` and `Cancel`; Send requests the final OS question rather than authorizing a network operation.

The OS question contains fixed wording and retained origin/model/input count/body bytes/pack ID/request ID. It explains that the endpoint may retain or forward text and that output is advisory. Only its affirmative custom result `Send selected text` permits dispatch. The source text and model reply are never placed in the OS alert. Native code binds the decision to the retained run/request and revalidates cancellation, profile generation and saved dependencies afterward.

The `main` capability cannot call review-window commands. Native command handlers additionally check the injected WebView label; no renderer parameter can impersonate that context. The review capability grants only the three review commands below, with no general window, event, filesystem, shell, HTTP, model configuration or sending permissions. Both windows keep the existing IPC-only CSP. A compromised main renderer can request a review but cannot complete its decision. A compromised review script still cannot bypass the final OS dialog. This does not eliminate UI deception or protect against compromised native code/OS, modified bundled assets or synthetic OS input. Closing the native review is cancellation, not consent.

## Main-window commands and envelope

The seven main-window commands and raw input limits remain those in [model-native-session.md](model-native-session.md). Every handler checks the injected label `main`, including no-input handlers. A raw JSON body is mandatory for configure/prepare/send/cancel; structured invoke objects are rejected for those operations.

Every main-window response has exactly these common fields:

- `schemaVersion`: `rangoon.local-session-result.v1`.
- `generation`: canonical nonnegative decimal string, or null only when native session state cannot be inspected.
- `runId`: `run:` plus 64 lowercase hexadecimal characters for an acquired operation, otherwise null. Async results retain their captured generation/run even if the current session changes.
- `authority`: `none`.
- `outcome` and only its outcome-specific fields below.

| Outcome | Additional fields | Meaning |
| --- | --- | --- |
| `unconfigured` / `configured` | `session`: published `SessionView` | Current profile inspection or successful configuration; active operation may be present |
| `cleared` | `session`: `SessionView` | Profile/request invalidated; an operation may still be exiting |
| `prepared` | `prepared`: published `PreparedView` | Retained exact request; no network performed |
| `checked` | `check`: published local `CheckResult` | Source-free protocol observation only |
| `completed` | `completion`: published local `Completion`; `freshness`: `current`, `stale` or `unavailable` | Validated advisory output; never application/review authority |
| `cancel_requested` | `session`: `SessionView` | Matching active run was signalled; not a claim that remote work stopped |
| `cancelled` | none | Operation finished without an accepted completion |
| `failed` | `code`: closed string below | No dynamic error text or provider body |

Closed codes are `invalid_request`, `invalid_profile`, `unconfigured`, `busy`, `session_unavailable`, `stale_prepared`, `run_not_found`, `input_unavailable`, `input_empty`, `input_stale`, `pack_invalid`, `pack_over_budget`, `workspace_busy`, `confirmation_unavailable`, plus transport `profile_mismatch`, `request_over_budget`, `timed_out`, `connection_failed`, `redirect_rejected`, `remote_rejected`, `response_too_large`, `response_invalid`, `response_incomplete`, `proposal_invalid`. A cancellation diagnostic uses outcome `cancelled`. Pre-lease errors have null run IDs. No promise is made about which overlapping invalid/busy condition is diagnosed first.

## Review-window commands

All three require injected label `model-confirmation`; a Rust-side pending record must match the active run and request. No network occurs in a review handler.

- `get_local_model_review`: no input. Returns `{schemaVersion:"rangoon.local-review.v1", outcome:"ready", authority:"none", generation, runId, requestId, origin, model, bodyBytes, bodyJson, packId, inputs}` from retained native state. `inputs` are the published `Dependency` records. Failure returns only `{schemaVersion, outcome:"unavailable", authority:"none"}`. Loading from the main window is rejected without metadata.
- `confirm_local_model_review`: at most 512 raw bytes, exact `{schemaVersion:"rangoon.local-review-decision.v1",runId,requestId}`. Claims the prompt once, then opens the final native OS dialog. The matching pending decision receives its affirmative/negative result only after the dialog exits. There is no `approved` field. The returned review status is `{schemaVersion:"rangoon.local-review-status.v1",outcome:"finished"|"unavailable",authority:"none"}` and confers no transport authority on the caller.
- `cancel_local_model_review`: the same closed decision input. Cancels the matching token and resolves a negative decision when no OS prompt is active. Returns the same review status. Invalid/mismatched/duplicate decisions never affect another run.

Only one review window/pending decision exists at a time. No model profile, payload, review decision or response is persisted. OS entropy and existing validated source-store reads remain the only non-network service dependencies. Opening the view is explicit; startup and profile configuration perform no model traffic.

## Orchestration and cleanup

Prepare reserves its session lease, then the shared native model-operation gate, before entering the guarded workspace read. Configure takes that shared gate briefly; check/send retain it through completion. Clear/cancel stay available. This gate is separate from the workspace guard and is required for the upcoming cloud route as well. It runs store work off the UI thread. Blocking callbacks own their leases, while an outer drop guard cancels work if its IPC waiter disappears. The existing workspace-operation guard is held only for bounded reads/packing, never dialogs or network. Failure consumes prior preparation according to the published session contract.

Check acquires a check lease, invokes the bounded `LocalClient::check`, and checks lease validity before returning. It neither discovers nor launches a model server.

Send consumes the matching prepared handle and installs a native pending consent record. It creates the review window and awaits its one-shot decision. Failure to create/load a usable confirmation surface fails closed or remains cancellable; it never assumes consent. After an affirmative OS result, native code checks lease validity, acquires the workspace guard, resolves every dependency through `Transmission::freshness`, and rejects stale/unavailable data before dispatch. The guard is released before awaiting transport. Cancellation is checked again and remains active throughout the bounded transport call.

After successful transport, native code rechecks lease validity and attempts a guarded dependency read. If the workspace is busy or the read cannot establish freshness, completion remains inspectable with `freshness:"unavailable"`. Stale/current results use the published comparison. Final cancellation/clear suppresses successful completion. A clear or cancel cannot recall already-transmitted bytes.

Native pending-state locks never span an await, store read or OS dialog. RAII cleanup removes only a matching pending run and closes its window. Closing a review before the OS prompt signals cancellation and resolves its negative decision. Closing/clearing/cancelling while the OS prompt is active only marks cancellation and prevents the review window from being destroyed until that prompt exits; a later affirmative answer cannot revive the run. Pending consent retains both the transmission lease and shared native gate until then, even if the send IPC waiter disappears. The blocking OS callback owns final decision/cleanup, including negative cleanup during unwinding; it does not depend on the confirm IPC waiter surviving its await. Freshness callbacks retain the same ownership until their reads return. Repeated prompt requests are rejected. Closing the main window invalidates model state and closes a nonprompting review. While an OS prompt is active, main-window closure is prevented until the prompt exits; the operator can close the main window afterward. No background transfer is retained across application exit.

The main UI polls profile inspection only while an explicit local operation is active, to obtain its native run ID for Cancel. It stops polling on terminal response and ignores replies from older UI epochs/profile generations. Route changes preserve an in-flight controller; clear/selection changes invalidate old payload display. The core reports operation kind, not whether an OS prompt or network exchange is active; the UI uses combined truthful waiting language and does not invent a Sending phase.

## Workbench and acceptance

Add `#model-assistance` to the actual native navigation, reusing Rangoon's existing icon, dark/light tokens, focus handling and reduced-motion behavior. The workbench has profile and saved-input controls, an exact payload/coverage/facts inspector, and advisory result/status panels. Narrow screens stack those panels in workflow order. Profiles are visibly session-only; endpoint reachability never becomes model availability, trust or local-only inference. Unknown tokens/cost/processing/retention remain unknown. The browser-only preview reports native unavailable and does not call any endpoint.

Selectors contain saved IDs and explicit revisions only; no arbitrary content or prompts. Classify/Decompose accept 1–16 records; Compare requires two capability revisions. Every input is fully protected. UI changes invalidate prepared controls, and native revalidation remains authoritative. Display source text, authored output, explanations, uncertainty and citations as inert text; no model-authored navigation, HTML, scripts or native invocation. Copy is a normal explicit clipboard action, not an approval boundary. No proposal application is introduced here.

Required evidence: native schema/raw-input and wrong-window rejection tests; single-prompt/matching-run/close/cancel/clear/cleanup tests; exact synthetic GET/POST with no POST before final consent; stale input refusal; cancellation and transport failures; controller stale-reply/unknown-outcome and view injection tests; keyboard, dark/light, narrow-width and reduced-motion review. Real isolated macOS GUI must exercise actual window/OS dialog and server receipt; Windows/Linux GUI qualification remains separately required. Source builds alone do not establish native confirmation readability or released support. Complete local/cloud V1, secret custody, durable profiles, proposal application and quality/token measurements remain open.
