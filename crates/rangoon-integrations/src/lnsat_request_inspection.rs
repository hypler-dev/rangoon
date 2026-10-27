//! Pure host lifecycle for one offline decoded LNSAT request record.
#![forbid(unsafe_code)]
use crate::lnsat_request_record::{
    RequestRecordBody, RequestRecordError, RequestRecordInput, RequestRecordInspection,
    inspect_request_record,
};
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub const VIEW_SCHEMA: &str = "rangoon.inspection.lnsat-request-summary-view.v1";
pub const MAX_PRESENTATION_BYTES: usize = 12_288;
const MAX_INPUT: usize = 65_536;
const MAX_SUMMARY: usize = 8_192;
const TERMINAL: u32 = u32::MAX;
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InspectionState {
    Empty,
    Inspecting,
    SummaryReady,
    Rejected,
    DiscardPending,
    Discarded,
    Closed,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InspectionControlError {
    Busy,
    Closed,
    StaleTicket,
    PresentationUnavailable,
}
impl InspectionControlError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Busy => "busy",
            Self::Closed => "closed",
            Self::StaleTicket => "stale_ticket",
            Self::PresentationUnavailable => "presentation_unavailable",
        }
    }
}
impl fmt::Display for InspectionControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
impl std::error::Error for InspectionControlError {}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionDisposition {
    Published,
    Dropped,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresentationError {
    OutputLimit,
    SerializationFailed,
}
impl fmt::Display for PresentationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::OutputLimit => "output_limit",
            Self::SerializationFailed => "serialization_failed",
        })
    }
}
impl std::error::Error for PresentationError {}
struct Owner(AtomicBool);
impl Owner {
    fn released(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    fn release(&self) {
        self.0.store(true, Ordering::Release)
    }
}
#[derive(Default, Clone)]
struct Binding {
    instance: Arc<()>,
    view: Arc<()>,
}
impl Binding {
    fn same(&self, x: &Self) -> bool {
        Arc::ptr_eq(&self.instance, &x.instance) && Arc::ptr_eq(&self.view, &x.view)
    }
}

/// Move-only trusted input custody. No payload accessor exists.
pub struct InspectionWork {
    binding: Binding,
    epoch: u32,
    owner: Option<Arc<Owner>>,
    input: Option<RequestRecordInput>,
}
impl fmt::Debug for InspectionWork {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("InspectionWork { input omitted }")
    }
}
impl InspectionWork {
    pub fn decode(self) -> InspectionCompletion {
        self.decode_with(inspect_request_record)
    }
    fn decode_with(
        mut self,
        decoder: impl FnOnce(RequestRecordInput) -> Result<RequestRecordInspection, RequestRecordError>,
    ) -> InspectionCompletion {
        let input = self.input.take().expect("owned input");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| decoder(input)))
            .map(DecodeResult::Decoded)
            .unwrap_or(DecodeResult::Panic);
        InspectionCompletion {
            binding: std::mem::take(&mut self.binding),
            epoch: self.epoch,
            owner: self.owner.take(),
            result: Some(result),
        }
    }
}
impl Drop for InspectionWork {
    fn drop(&mut self) {
        drop(self.input.take());
        if let Some(owner) = self.owner.take() {
            owner.release()
        }
    }
}
/// Move-only completed decoder custody. It retains decoder result until accepted or dropped.
pub struct InspectionCompletion {
    binding: Binding,
    epoch: u32,
    owner: Option<Arc<Owner>>,
    result: Option<DecodeResult>,
}
impl fmt::Debug for InspectionCompletion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("InspectionCompletion { result omitted }")
    }
}
impl Drop for InspectionCompletion {
    fn drop(&mut self) {
        drop(self.result.take());
        if let Some(owner) = self.owner.take() {
            owner.release()
        }
    }
}
enum DecodeResult {
    Decoded(Result<RequestRecordInspection, RequestRecordError>),
    Panic,
}
struct PurgeId;
struct Pending {
    ticket: Arc<PurgeId>,
    owner: Option<Arc<Owner>>,
    ack: bool,
}
#[derive(Clone)]
pub struct PurgeTicket {
    binding: Binding,
    ticket: Arc<PurgeId>,
}
impl fmt::Debug for PurgeTicket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PurgeTicket { opaque: true }")
    }
}

pub struct RequestInspectionHost {
    binding: Binding,
    epoch: u32,
    state: InspectionState,
    closed: bool,
    active: Option<Arc<Owner>>,
    presentation: Option<View>,
    pending: Option<Pending>,
}
impl Default for RequestInspectionHost {
    fn default() -> Self {
        Self {
            binding: Binding::default(),
            epoch: 1,
            state: InspectionState::Empty,
            closed: false,
            active: None,
            presentation: None,
            pending: None,
        }
    }
}
impl fmt::Debug for RequestInspectionHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RequestInspectionHost")
            .field("state", &self.state)
            .field("epoch", &self.epoch)
            .finish()
    }
}
impl RequestInspectionHost {
    pub fn new() -> Self {
        Self::default()
    }
    pub const fn state(&self) -> InspectionState {
        self.state
    }
    pub const fn epoch(&self) -> u32 {
        self.epoch
    }
    pub fn begin(
        &mut self,
        input: RequestRecordInput,
    ) -> Result<InspectionWork, InspectionControlError> {
        self.settle();
        if self.closed {
            return Err(InspectionControlError::Closed);
        }
        if !matches!(
            self.state,
            InspectionState::Empty | InspectionState::Discarded
        ) {
            return Err(InspectionControlError::Busy);
        }
        if input_size(&input) > MAX_INPUT {
            return Err(InspectionControlError::PresentationUnavailable);
        }
        let owner = Arc::new(Owner(AtomicBool::new(false)));
        self.active = Some(owner.clone());
        self.presentation = None;
        self.state = InspectionState::Inspecting;
        Ok(InspectionWork {
            binding: Binding {
                instance: self.binding.instance.clone(),
                view: self.binding.view.clone(),
            },
            epoch: self.epoch,
            owner: Some(owner),
            input: Some(input),
        })
    }
    pub fn complete(&mut self, mut done: InspectionCompletion) -> CompletionDisposition {
        let owner = done.owner.clone();
        let valid = owner.as_ref().is_some_and(|o| {
            self.state == InspectionState::Inspecting
                && self.epoch == done.epoch
                && self.binding.same(&done.binding)
                && self.active.as_ref().is_some_and(|a| Arc::ptr_eq(a, o))
        });
        let result = done.result.take();
        if !valid {
            drop(result);
            drop(done);
            self.settle();
            return CompletionDisposition::Dropped;
        }
        let view = match result.expect("one result") {
            DecodeResult::Decoded(Ok(inspection)) => {
                match decode_summary(inspection.summary_json()) {
                    Ok(summary) => View::summary(self.epoch, summary),
                    Err(_) => View::integration(self.epoch, "summary_rejected"),
                }
            }
            DecodeResult::Decoded(Err(error)) => View::decoder(self.epoch, error),
            DecodeResult::Panic => View::integration(self.epoch, "inspection_failed"),
        };
        self.presentation = Some(view);
        self.active = None;
        self.state = if self.presentation.as_ref().is_some_and(View::summary_ready) {
            InspectionState::SummaryReady
        } else {
            InspectionState::Rejected
        };
        drop(done);
        CompletionDisposition::Published
    }
    pub fn discard(&mut self) -> Option<PurgeTicket> {
        // An already pending cycle may finish here, but this same call must not
        // allocate a replacement cycle after observing its final owner release.
        if self.pending.is_some() {
            self.settle();
            return self.ticket();
        }
        self.settle();
        if self.state == InspectionState::Closed {
            return None;
        }
        if self.state != InspectionState::DiscardPending {
            self.purge(false)
        }
        self.ticket()
    }
    pub fn close(&mut self) -> Option<PurgeTicket> {
        self.closed = true;
        self.settle();
        match self.state {
            InspectionState::Closed => None,
            InspectionState::Discarded => {
                self.state = InspectionState::Closed;
                None
            }
            InspectionState::DiscardPending => self.ticket(),
            _ => {
                self.purge(true);
                self.ticket()
            }
        }
    }
    pub fn acknowledge_purge(&mut self, t: PurgeTicket) -> Result<(), InspectionControlError> {
        let Some(p) = self.pending.as_mut() else {
            return Err(InspectionControlError::StaleTicket);
        };
        if !self.binding.same(&t.binding) || !Arc::ptr_eq(&p.ticket, &t.ticket) {
            return Err(InspectionControlError::StaleTicket);
        }
        p.ack = true;
        self.settle();
        Ok(())
    }
    pub fn refresh_cleanup(&mut self) {
        self.settle()
    }
    pub fn presentation(&mut self) -> Result<PresentationLease<'_>, InspectionControlError> {
        self.settle();
        if !matches!(
            self.state,
            InspectionState::SummaryReady | InspectionState::Rejected
        ) {
            return Err(InspectionControlError::PresentationUnavailable);
        }
        Ok(PresentationLease {
            view: self.presentation.as_ref().expect("visible view"),
        })
    }
    fn ticket(&self) -> Option<PurgeTicket> {
        self.pending.as_ref().map(|p| PurgeTicket {
            binding: Binding {
                instance: self.binding.instance.clone(),
                view: self.binding.view.clone(),
            },
            ticket: p.ticket.clone(),
        })
    }
    fn purge(&mut self, closing: bool) {
        self.presentation = None;
        if self.epoch >= TERMINAL - 1 {
            self.epoch = TERMINAL;
            self.closed = true
        } else {
            self.epoch += 1
        }
        if closing {
            self.closed = true
        }
        self.pending = Some(Pending {
            ticket: Arc::new(PurgeId),
            owner: self.active.take(),
            ack: false,
        });
        self.state = InspectionState::DiscardPending;
        self.settle()
    }
    fn settle(&mut self) {
        if self
            .pending
            .as_ref()
            .is_some_and(|p| p.ack && p.owner.as_ref().is_none_or(|o| o.released()))
        {
            self.pending = None;
            self.state = if self.closed {
                InspectionState::Closed
            } else {
                InspectionState::Discarded
            }
        }
    }
}
/// Borrowed view access serializes delivery against discard/close. No callback is invoked.
/// A caller cannot mutate the same host while it still uses the delivery lease.
/// Copied presentation bytes are outside this borrow and require cooperative purge.
///
/// ```compile_fail,E0499
/// use rangoon_integrations::lnsat_request_inspection::RequestInspectionHost;
/// let mut host = RequestInspectionHost::new();
/// let lease = host.presentation().unwrap();
/// host.discard();
/// let _ = lease.json_bytes();
/// ```
pub struct PresentationLease<'a> {
    view: &'a View,
}
impl PresentationLease<'_> {
    pub fn json_bytes(&self) -> Result<Vec<u8>, PresentationError> {
        checked_encode(self.view, MAX_PRESENTATION_BYTES)
    }
}
impl fmt::Debug for PresentationLease<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PresentationLease { summary only }")
    }
}
fn checked_encode<T: Serialize>(value: &T, limit: usize) -> Result<Vec<u8>, PresentationError> {
    let bytes = serde_json::to_vec(value).map_err(|_| PresentationError::SerializationFailed)?;
    if bytes.len() > limit {
        Err(PresentationError::OutputLimit)
    } else {
        Ok(bytes)
    }
}

fn input_size(i: &RequestRecordInput) -> usize {
    match &i.body {
        RequestRecordBody::Body200(x) | RequestRecordBody::Refused403(x) => x.len(),
        RequestRecordBody::ReadUnavailable(_) => 0,
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct View {
    schema_version: &'static str,
    epoch: u32,
    state: InspectionState,
    summary: Option<Summary>,
    error: Option<LocalError>,
}
impl View {
    fn summary(epoch: u32, summary: Summary) -> Self {
        Self {
            schema_version: VIEW_SCHEMA,
            epoch,
            state: InspectionState::SummaryReady,
            summary: Some(summary),
            error: None,
        }
    }
    fn decoder(epoch: u32, error: RequestRecordError) -> Self {
        Self {
            schema_version: VIEW_SCHEMA,
            epoch,
            state: InspectionState::Rejected,
            summary: None,
            error: Some(LocalError {
                kind: Kind::Decoder,
                code: error.code(),
                truth: Truth::fixed(),
            }),
        }
    }
    fn integration(epoch: u32, code: &'static str) -> Self {
        Self {
            schema_version: VIEW_SCHEMA,
            epoch,
            state: InspectionState::Rejected,
            summary: None,
            error: Some(LocalError {
                kind: Kind::Integration,
                code,
                truth: Truth::fixed(),
            }),
        }
    }
    fn summary_ready(&self) -> bool {
        self.summary.is_some()
    }
}
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Decoder,
    Integration,
}
#[derive(Serialize)]
struct LocalError {
    kind: Kind,
    code: &'static str,
    truth: Truth,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Summary {
    schema_version: Schema,
    record_availability: Availability,
    reported_evaluation: ReportedEvaluation,
    reported_policy: Policy,
    read_failure: Option<ReadFailure>,
    security_activity_touched: Option<bool>,
    family_counts: Option<Counts>,
    truth: Truth,
    omissions: Vec<Omission>,
}
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
enum Schema {
    #[serde(rename = "rangoon.integrations.lnsat-request-summary.v1")]
    V1,
}
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Availability {
    Complete,
    Degraded,
    ReadRejectedUnclassified,
    ReadUnavailable,
}
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ReportedEvaluation {
    PolicyRecorded,
    EvaluationFailed,
    EvaluationUnknown,
    NotObserved,
}
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Policy {
    Allow,
    Deny,
    ApprovalRequired,
    NotObserved,
}
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ReadFailure {
    NativeReadRejectedUnclassified,
    ReadNotConfigured,
    ReadTransportUnavailable,
    ReadCancelled,
    ReadTimeout,
    ReadUnauthenticated,
    ReadUnsupportedTransport,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Counts {
    approvals: u64,
    approval_decisions: u64,
    authorization_attempts: u64,
    authorizations: u64,
    operations: u64,
    consumptions: u64,
    operation_attempts: u64,
    receipts: u64,
    reconciliations: u64,
    states: u64,
}
impl Counts {
    fn ok(&self) -> bool {
        [
            self.approvals,
            self.approval_decisions,
            self.authorization_attempts,
            self.authorizations,
            self.operations,
            self.consumptions,
            self.operation_attempts,
            self.receipts,
            self.reconciliations,
            self.states,
        ]
        .into_iter()
        .all(|x| x <= 64)
    }
}
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Truth {
    authenticity: Auth,
    authority: Authority,
    execution: Execution,
    authoritative_outcome: Outcome,
    independent_effect: Effect,
    currentness: Current,
}
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Auth {
    Unverified,
}
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Authority {
    None,
}
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Execution {
    Unavailable,
}
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Outcome {
    Unknown,
}
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Effect {
    NotObserved,
}
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Current {
    Unverified,
}
impl Truth {
    fn fixed() -> Self {
        Self {
            authenticity: Auth::Unverified,
            authority: Authority::None,
            execution: Execution::Unavailable,
            authoritative_outcome: Outcome::Unknown,
            independent_effect: Effect::NotObserved,
            currentness: Current::Unverified,
        }
    }
}
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Omission {
    path: Path,
    reason: Reason,
    disposition: Disposition,
}
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
enum Path {
    #[serde(rename = "request.requester_ref")]
    Requester,
    #[serde(rename = "evidence.approval_decisions[*].reason")]
    ApprovalReason,
}
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
enum Reason {
    #[serde(rename = "privacy_projection")]
    Privacy,
}
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
enum Disposition {
    #[serde(rename = "policy_omission_not_presence_claim")]
    Policy,
}
fn omissions() -> Vec<Omission> {
    vec![
        Omission {
            path: Path::Requester,
            reason: Reason::Privacy,
            disposition: Disposition::Policy,
        },
        Omission {
            path: Path::ApprovalReason,
            reason: Reason::Privacy,
            disposition: Disposition::Policy,
        },
    ]
}
fn decode_summary(b: &[u8]) -> Result<Summary, ()> {
    if b.len() > MAX_SUMMARY {
        return Err(());
    }
    NoDuplicate::new(b).check()?;
    let s: Summary = serde_json::from_slice(b).map_err(|_| ())?;
    if s.truth != Truth::fixed()
        || s.omissions != omissions()
        || !s.family_counts.as_ref().is_none_or(Counts::ok)
    {
        return Err(());
    }
    match (
        &s.record_availability,
        &s.reported_evaluation,
        &s.reported_policy,
        &s.read_failure,
        &s.security_activity_touched,
        &s.family_counts,
    ) {
        (
            Availability::Complete,
            ReportedEvaluation::PolicyRecorded,
            Policy::Allow | Policy::Deny | Policy::ApprovalRequired,
            None,
            Some(_),
            Some(_),
        )
        | (
            Availability::Complete,
            ReportedEvaluation::EvaluationFailed | ReportedEvaluation::EvaluationUnknown,
            Policy::NotObserved,
            None,
            Some(_),
            None,
        )
        | (
            Availability::Degraded,
            ReportedEvaluation::EvaluationUnknown,
            Policy::NotObserved,
            None,
            Some(_),
            None,
        )
        | (
            Availability::ReadRejectedUnclassified,
            ReportedEvaluation::NotObserved,
            Policy::NotObserved,
            Some(ReadFailure::NativeReadRejectedUnclassified),
            None,
            None,
        )
        | (
            Availability::ReadUnavailable,
            ReportedEvaluation::NotObserved,
            Policy::NotObserved,
            Some(
                ReadFailure::ReadNotConfigured
                | ReadFailure::ReadTransportUnavailable
                | ReadFailure::ReadCancelled
                | ReadFailure::ReadTimeout
                | ReadFailure::ReadUnauthenticated
                | ReadFailure::ReadUnsupportedTransport,
            ),
            None,
            None,
        ) => Ok(s),
        _ => Err(()),
    }
}
struct NoDuplicate<'a> {
    b: &'a [u8],
    p: usize,
    root_keys: Vec<String>,
}
impl<'a> NoDuplicate<'a> {
    fn new(b: &'a [u8]) -> Self {
        Self {
            b,
            p: 0,
            root_keys: Vec::new(),
        }
    }
    fn check(mut self) -> Result<(), ()> {
        self.ws();
        self.v(0)?;
        self.ws();
        const FIELDS: [&str; 9] = [
            "schemaVersion",
            "recordAvailability",
            "reportedEvaluation",
            "reportedPolicy",
            "readFailure",
            "securityActivityTouched",
            "familyCounts",
            "truth",
            "omissions",
        ];
        if self.p == self.b.len()
            && self.root_keys.len() == FIELDS.len()
            && FIELDS
                .iter()
                .all(|field| self.root_keys.iter().any(|key| key == field))
        {
            Ok(())
        } else {
            Err(())
        }
    }
    fn v(&mut self, d: u8) -> Result<(), ()> {
        if d >= 16 {
            return Err(());
        }
        self.ws();
        match self.peek() {
            Some(b'{') => self.obj(d + 1),
            Some(b'[') => self.arr(d + 1),
            Some(b'\"') => self.str().map(|_| ()),
            Some(b't') => self.word(b"true"),
            Some(b'f') => self.word(b"false"),
            Some(b'n') => self.word(b"null"),
            Some(b'-' | b'0'..=b'9') => {
                let s = self.p;
                while matches!(
                    self.peek(),
                    Some(b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E')
                ) {
                    self.p += 1
                }
                serde_json::from_slice::<serde_json::Number>(&self.b[s..self.p])
                    .map(|_| ())
                    .map_err(|_| ())
            }
            _ => Err(()),
        }
    }
    fn obj(&mut self, d: u8) -> Result<(), ()> {
        self.p += 1;
        let mut k = Vec::new();
        self.ws();
        if self.take(b'}') {
            return Ok(());
        }
        loop {
            self.ws();
            let x = self.str()?;
            if k.iter().any(|y: &String| y == &x) || k.len() >= 32 {
                return Err(());
            }
            k.push(x);
            self.ws();
            if !self.take(b':') {
                return Err(());
            }
            self.v(d)?;
            self.ws();
            if self.take(b'}') {
                if d == 1 {
                    self.root_keys = k;
                }
                return Ok(());
            }
            if !self.take(b',') {
                return Err(());
            }
        }
    }
    fn arr(&mut self, d: u8) -> Result<(), ()> {
        self.p += 1;
        let mut n = 0;
        self.ws();
        if self.take(b']') {
            return Ok(());
        }
        loop {
            n += 1;
            if n > 64 {
                return Err(());
            }
            self.v(d)?;
            self.ws();
            if self.take(b']') {
                return Ok(());
            }
            if !self.take(b',') {
                return Err(());
            }
        }
    }
    fn str(&mut self) -> Result<String, ()> {
        let s = self.p;
        if !self.take(b'\"') {
            return Err(());
        }
        let mut esc = false;
        while let Some(x) = self.peek() {
            self.p += 1;
            if esc {
                esc = false;
                continue;
            }
            if x == b'\\' {
                esc = true
            } else if x == b'\"' {
                let decoded: String = serde_json::from_slice(&self.b[s..self.p]).map_err(|_| ())?;
                return if decoded.len() <= 4096 {
                    Ok(decoded)
                } else {
                    Err(())
                };
            } else if x < 32 {
                return Err(());
            }
        }
        Err(())
    }
    fn word(&mut self, x: &[u8]) -> Result<(), ()> {
        if self.b.get(self.p..self.p + x.len()) == Some(x) {
            self.p += x.len();
            Ok(())
        } else {
            Err(())
        }
    }
    fn ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.p += 1
        }
    }
    fn peek(&self) -> Option<u8> {
        self.b.get(self.p).copied()
    }
    fn take(&mut self, x: u8) -> bool {
        if self.peek() == Some(x) {
            self.p += 1;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod contract_tests {
    use super::*;
    use crate::lnsat_request_record::ReadUnavailable;
    use serde_json::{Value, json};
    fn input() -> RequestRecordInput {
        RequestRecordInput {
            installation_id: "sensitive-installation-canary".into(),
            environment: "sensitive-environment-canary".into(),
            selected_request_id: format!("arq_{:064x}", 1),
            expected_project_ref: "project:secret".into(),
            expected_resource_ref: "resource:secret".into(),
            body: RequestRecordBody::ReadUnavailable(ReadUnavailable::Timeout),
            read_at: None,
        }
    }
    fn summary() -> Value {
        json!({"schemaVersion":"rangoon.integrations.lnsat-request-summary.v1","recordAvailability":"read_unavailable","reportedEvaluation":"not_observed","reportedPolicy":"not_observed","readFailure":"read_timeout","securityActivityTouched":null,"familyCounts":null,"truth":{"authenticity":"unverified","authority":"none","execution":"unavailable","authoritativeOutcome":"unknown","independentEffect":"not_observed","currentness":"unverified"},"omissions":[{"path":"request.requester_ref","reason":"privacy_projection","disposition":"policy_omission_not_presence_claim"},{"path":"evidence.approval_decisions[*].reason","reason":"privacy_projection","disposition":"policy_omission_not_presence_claim"}]})
    }
    fn valid(value: &Value) -> bool {
        decode_summary(&serde_json::to_vec(value).unwrap()).is_ok()
    }
    fn counts(n: u64) -> Value {
        json!({"approvals":n,"approval_decisions":n,"authorization_attempts":n,"authorizations":n,"operations":n,"consumptions":n,"operation_attempts":n,"receipts":n,"reconciliations":n,"states":n})
    }
    fn policy() -> Value {
        let mut v = summary();
        v["recordAvailability"] = json!("complete");
        v["reportedEvaluation"] = json!("policy_recorded");
        v["reportedPolicy"] = json!("allow");
        v["readFailure"] = Value::Null;
        v["securityActivityTouched"] = json!(false);
        v["familyCounts"] = counts(0);
        v
    }

    #[test]
    fn i10_all_thirteen_typed_errors_have_exact_local_envelope() {
        use RequestRecordError::*;
        let table = [
            (InputLimit, "input_limit"),
            (InvalidEncoding, "invalid_encoding"),
            (InvalidJson, "invalid_json"),
            (DuplicateField, "duplicate_field"),
            (StructuralLimit, "structural_limit"),
            (UnsupportedSchema, "unsupported_schema"),
            (InvalidEnvelope, "invalid_envelope"),
            (SelectionMismatch, "selection_mismatch"),
            (ScopeMismatch, "scope_mismatch"),
            (LineageConflict, "lineage_conflict"),
            (UnsupportedState, "unsupported_state"),
            (OutputLimit, "output_limit"),
            (SerializationFailed, "serialization_failed"),
        ];
        for (error, code) in table {
            let v = View::decoder(7, error);
            let actual: Value =
                serde_json::from_slice(&checked_encode(&v, MAX_PRESENTATION_BYTES).unwrap())
                    .unwrap();
            assert_eq!(
                actual,
                json!({"schemaVersion":VIEW_SCHEMA,"epoch":7,"state":"rejected","summary":null,"error":{"kind":"decoder","code":code,"truth":summary()["truth"]}})
            );
            let native: Value = serde_json::from_slice(&error.json_bytes()).unwrap();
            assert_eq!(
                native,
                json!({"schemaVersion":"rangoon.integrations.lnsat-request-error.v1","code":code,"truth":summary()["truth"]})
            );
            assert!(error.json_bytes().len() <= 1024);
        }
    }
    #[test]
    fn i11_exact_keys_required_nulls_duplicates_and_types() {
        nested_duplicate_oracles();
        let base = summary();
        assert!(valid(&base));
        for key in base.as_object().unwrap().keys() {
            let mut v = base.clone();
            v.as_object_mut().unwrap().remove(key);
            assert!(!valid(&v), "missing {key}");
        }
        for (key, value) in [
            ("schemaVersion", json!("unknown")),
            ("recordAvailability", json!(false)),
            ("reportedPolicy", json!("execute")),
            ("readFailure", json!({})),
            ("securityActivityTouched", json!(0)),
            ("familyCounts", json!([])),
        ] {
            let mut v = base.clone();
            v[key] = value;
            assert!(!valid(&v), "type {key}");
        }
        let bytes = serde_json::to_string(&base).unwrap();
        for key in ["schemaVersion", "truth"] {
            let value = serde_json::to_string(&base[key]).unwrap();
            let duplicate = format!("{{\"{key}\":{value},{}", &bytes[1..]);
            assert!(decode_summary(duplicate.as_bytes()).is_err());
        }
        let escaped = format!(
            "{{\"\\u0073chemaVersion\":\"rangoon.integrations.lnsat-request-summary.v1\",{}",
            &bytes[1..]
        );
        assert!(decode_summary(escaped.as_bytes()).is_err());
        let mut v = base.clone();
        v["truth"]["unexpected"] = json!("secret");
        assert!(!valid(&v));
        let mut v = base;
        v["unexpected"] = json!("secret");
        assert!(!valid(&v));
    }
    // Continue I11 with nested duplicates: a lossy JSON map would accept these.
    // The originals and duplicate spellings are constructed independently of
    // the production parser, including JSON-escaped keys.
    fn nested_duplicate_oracles() {
        let original = serde_json::to_string(&policy()).unwrap();
        let cases = [
            (r#""truth":{"#, r#""truth":{"authority":"none","#),
            (r#""truth":{"#, r#""truth":{"\u0061uthority":"none","#),
            (r#""familyCounts":{"#, r#""familyCounts":{"approvals":0,"#),
            (
                r#""familyCounts":{"#,
                r#""familyCounts":{"\u0061pprovals":0,"#,
            ),
            (
                r#""omissions":[{"#,
                r#""omissions":[{"path":"request.requester_ref","#,
            ),
            (
                r#""omissions":[{"#,
                r#""omissions":[{"\u0070ath":"request.requester_ref","#,
            ),
        ];
        assert!(decode_summary(original.as_bytes()).is_ok());
        for (target, replacement) in cases {
            assert!(original.contains(target));
            let duplicate = original.replacen(target, replacement, 1);
            assert!(
                decode_summary(duplicate.as_bytes()).is_err(),
                "nested duplicate accepted"
            );
        }
    }
    #[test]
    fn i13_counts_truth_omissions_and_impossible_tuples() {
        let base = policy();
        assert!(valid(&base));
        for n in [0, 64] {
            let mut v = base.clone();
            v["familyCounts"] = counts(n);
            assert!(valid(&v));
        }
        for key in counts(0).as_object().unwrap().keys() {
            for bad in [json!(65), json!(-1), json!(true), json!(1.0), Value::Null] {
                let mut v = base.clone();
                v["familyCounts"][key] = bad;
                assert!(!valid(&v));
            }
            let mut v = base.clone();
            v["familyCounts"].as_object_mut().unwrap().remove(key);
            assert!(!valid(&v));
        }
        let mut v = base.clone();
        v["familyCounts"]["approvalDecisions"] = json!(0);
        assert!(!valid(&v));
        let mut v = base.clone();
        v["familyCounts"] = Value::Null;
        assert!(!valid(&v));
        let mut v = summary();
        v["familyCounts"] = counts(0);
        assert!(!valid(&v));
        for key in summary()["truth"].as_object().unwrap().keys() {
            let mut v = base.clone();
            v["truth"][key] = json!("verified");
            assert!(!valid(&v));
        }
        for change in 0..5 {
            let mut v = base.clone();
            match change {
                0 => v["omissions"].as_array_mut().unwrap().reverse(),
                1 => {
                    v["omissions"].as_array_mut().unwrap().pop();
                }
                2 => v["omissions"][0]["reason"] = json!("other"),
                3 => v["omissions"][0]["disposition"] = json!("present"),
                _ => v["omissions"][0]["path"] = json!("other"),
            };
            assert!(!valid(&v));
        }
        for availability in ["degraded", "read_unavailable", "read_rejected_unclassified"] {
            let mut v = base.clone();
            v["recordAvailability"] = json!(availability);
            assert!(!valid(&v));
        }
        let mut v = summary();
        v["readFailure"] = json!("native_read_rejected_unclassified");
        assert!(!valid(&v));
    }
    #[test]
    fn i14_exact_input_and_context_boundaries() {
        for size in [65536, 65537] {
            let mut h = RequestInspectionHost::new();
            let mut i = input();
            i.body = RequestRecordBody::Body200(vec![b' '; size]);
            let admitted = h.begin(i);
            assert_eq!(admitted.is_ok(), size == 65536);
        }
        for n in [128, 129] {
            let mut h = RequestInspectionHost::new();
            let mut i = input();
            i.installation_id = "x".repeat(n);
            let done = h.begin(i).unwrap().decode();
            h.complete(done);
            let bytes = h.presentation().unwrap().json_bytes().unwrap();
            let v: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(
                v["state"],
                if n == 128 {
                    "summary_ready"
                } else {
                    "rejected"
                }
            );
        }
    }
    #[test]
    fn i15_summary_size_and_structural_limits() {
        let bytes = serde_json::to_vec(&summary()).unwrap();
        let mut exact = bytes.clone();
        exact.resize(MAX_SUMMARY, b' ');
        assert!(decode_summary(&exact).is_ok());
        exact.push(b' ');
        assert!(decode_summary(&exact).is_err());
        let pretty = serde_json::to_vec_pretty(&summary()).unwrap();
        assert!(decode_summary(&pretty).is_ok());
        let mut v = summary();
        v["unexpected"] = json!("x".repeat(4097));
        assert!(!valid(&v));
        let deep = format!("{}0{}", "[".repeat(17), "]".repeat(17));
        assert!(decode_summary(deep.as_bytes()).is_err());
        let mut v = summary();
        v["omissions"] = json!([]);
        assert!(!valid(&v));
    }
    #[test]
    fn i16_presentation_cap_and_serializer_failure() {
        let exact = "x".repeat(MAX_PRESENTATION_BYTES - 2);
        assert_eq!(
            checked_encode(&exact, MAX_PRESENTATION_BYTES)
                .unwrap()
                .len(),
            MAX_PRESENTATION_BYTES
        );
        assert_eq!(
            checked_encode(&(exact + "x"), MAX_PRESENTATION_BYTES).unwrap_err(),
            PresentationError::OutputLimit
        );
        struct Fail;
        impl Serialize for Fail {
            fn serialize<S: serde::Serializer>(&self, _s: S) -> Result<S::Ok, S::Error> {
                Err(serde::ser::Error::custom("sensitive serializer canary"))
            }
        }
        let err = checked_encode(&Fail, MAX_PRESENTATION_BYTES).unwrap_err();
        assert_eq!(err, PresentationError::SerializationFailed);
        assert!(!format!("{err:?} {err}").contains("canary"));
    }
    #[test]
    fn i21_ack_cannot_release_held_completed_result_or_foreign_owner() {
        let mut a = RequestInspectionHost::new();
        let own = a.begin(input()).unwrap().decode();
        let t = a.discard().unwrap();
        a.acknowledge_purge(t).unwrap();
        assert_eq!(a.state(), InspectionState::DiscardPending);
        let mut b = RequestInspectionHost::new();
        let foreign = b.begin(input()).unwrap().decode();
        assert_eq!(a.complete(foreign), CompletionDisposition::Dropped);
        assert_eq!(a.state(), InspectionState::DiscardPending);
        drop(own);
        a.refresh_cleanup();
        assert_eq!(a.state(), InspectionState::Discarded);
        assert!(a.presentation().is_err());
    }
    #[test]
    fn i23_i24_exact_cycle_and_view_join() {
        let mut h = RequestInspectionHost::new();
        let t = h.discard().unwrap();
        let repeated = h.discard().unwrap();
        assert!(Arc::ptr_eq(&t.ticket, &repeated.ticket));
        h.close();
        assert!(Arc::ptr_eq(&t.ticket, &h.ticket().unwrap().ticket));
        h.acknowledge_purge(t.clone()).unwrap();
        assert_eq!(h.state(), InspectionState::Closed);
        let mut h = RequestInspectionHost::new();
        let old = h.discard().unwrap();
        h.acknowledge_purge(old.clone()).unwrap();
        let current = h.discard().unwrap();
        assert!(h.acknowledge_purge(old).is_err());
        assert_eq!(h.state(), InspectionState::DiscardPending);
        let wrong_view = PurgeTicket {
            binding: Binding {
                instance: h.binding.instance.clone(),
                view: Arc::new(()),
            },
            ticket: current.ticket.clone(),
        };
        assert!(h.acknowledge_purge(wrong_view).is_err());
        h.acknowledge_purge(current).unwrap();
    }
    #[test]
    fn i26_terminal_generation_reserve_does_not_wrap() {
        let mut h = RequestInspectionHost::new();
        h.epoch = TERMINAL - 1;
        let done = h.begin(input()).unwrap().decode();
        let t = h.discard().unwrap();
        assert_eq!(h.epoch(), TERMINAL);
        assert_eq!(
            h.begin(input()).unwrap_err(),
            InspectionControlError::Closed
        );
        h.acknowledge_purge(t).unwrap();
        assert_eq!(h.state(), InspectionState::DiscardPending);
        assert_eq!(h.complete(done), CompletionDisposition::Dropped);
        assert_eq!(h.state(), InspectionState::Closed);
        assert_eq!(h.epoch(), TERMINAL);
    }
    #[test]
    fn i27_decoder_panic_has_no_result_or_retry_authority() {
        let mut h = RequestInspectionHost::new();
        let w = h.begin(input()).unwrap();
        let done = w.decode_with(|_| panic!("synthetic decoder failure"));
        let t = h.discard().unwrap();
        h.acknowledge_purge(t).unwrap();
        assert_eq!(h.state(), InspectionState::DiscardPending);
        h.complete(done);
        assert_eq!(h.state(), InspectionState::Discarded);
        assert!(h.presentation().is_err());
        let mut h = RequestInspectionHost::new();
        let done = h
            .begin(input())
            .unwrap()
            .decode_with(|_| panic!("synthetic decoder failure"));
        h.complete(done);
        let v: Value =
            serde_json::from_slice(&h.presentation().unwrap().json_bytes().unwrap()).unwrap();
        assert_eq!(v["error"]["code"], "inspection_failed");
        assert!(v["summary"].is_null());
        assert_eq!(h.begin(input()).unwrap_err(), InspectionControlError::Busy);
    }
    #[test]
    fn i19_retained_projection_is_cleared_immediately() {
        let mut h = RequestInspectionHost::new();
        let done = h.begin(input()).unwrap().decode();
        h.complete(done);
        assert!(h.presentation.is_some());
        let epoch = h.epoch();
        let _ticket = h.discard().unwrap();
        assert_eq!(h.epoch(), epoch + 1);
        assert!(h.presentation.is_none());
        assert!(h.presentation().is_err());
        assert_eq!(h.state(), InspectionState::DiscardPending);
    }
    #[test]
    fn i24_pending_completion_does_not_allocate_replacement_purge() {
        let mut h = RequestInspectionHost::new();
        let held = h.begin(input()).unwrap();
        let t = h.discard().unwrap();
        let epoch = h.epoch();
        h.acknowledge_purge(t).unwrap();
        drop(held);
        assert!(h.discard().is_none());
        assert_eq!(h.state(), InspectionState::Discarded);
        assert_eq!(h.epoch(), epoch);
    }
    #[test]
    fn i28_production_surface_remains_pure_and_unmounted() {
        let source = include_str!("lnsat_request_inspection.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        for forbidden in [
            "std::fs",
            "std::process",
            "std::thread",
            "std::net",
            "std::env",
            "SystemTime",
            "Instant::now",
            "pub fn decode_with",
            "pub fn set_epoch",
            "pub fn decode_summary",
        ] {
            assert!(!source.contains(forbidden), "unexpected port {forbidden}");
        }
        assert!(source.contains("pub fn presentation(&mut self)"));
        assert!(source.contains("owner.release()"));
    }
}
