use std::cell::Cell;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use adoc_core::{AgentJsonRelations, ExpiresInfo, RetrievalRecord, RetrievalSource};
use adoc_core::{Clock, ExplainError, ExplainService, RecordResolver, ResolverError};
use chrono::NaiveDate;

// ---------------------------------------------------------------------------
// Fakes
// ---------------------------------------------------------------------------

/// Deterministic clock for service tests.
///
/// `today()` always returns 2026-05-08.
///
/// `now_instant()` returns successive instants spaced `step` apart.  A call
/// counter is maintained in a `Cell` so the fake can be used behind a shared
/// reference, matching the `Clock` trait which takes `&self`.
///
/// The implementation uses a real `Instant::now()` baseline captured once at
/// construction time, then adds multiples of `step` on each call.  This keeps
/// `Instant` opaque (no public constructor) while still being deterministic in
/// terms of *elapsed duration between calls*.
struct FakeClock {
    baseline: Instant,
    step: Duration,
    call_count: Cell<u32>,
}

impl FakeClock {
    /// Construct a clock whose `now_instant()` advances by `step` per call.
    fn with_step(step: Duration) -> Self {
        Self {
            baseline: Instant::now(),
            step,
            call_count: Cell::new(0),
        }
    }

    /// Construct a clock with zero-advance (both snapshots return the same
    /// instant, so `duration` will be `Duration::ZERO`).
    fn zero() -> Self {
        Self::with_step(Duration::ZERO)
    }
}

impl Clock for FakeClock {
    fn today(&self) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 5, 8).expect("valid date")
    }

    fn now_instant(&self) -> Instant {
        let count = self.call_count.get();
        self.call_count.set(count + 1);
        // Return baseline + count * step so successive calls are monotonically
        // increasing by exactly `step`.
        self.baseline + self.step * count
    }
}

struct FakeResolver {
    /// Map of id -> record (None means "found but status is None", absent means
    /// "resolve returns None" i.e. not-found).
    records: BTreeMap<String, Option<RetrievalRecord>>,
    /// When Some(_), every call returns this error instead.
    error: Option<ResolverError>,
}

impl FakeResolver {
    fn new() -> Self {
        Self {
            records: BTreeMap::new(),
            error: None,
        }
    }

    fn with_record(mut self, record: RetrievalRecord) -> Self {
        self.records.insert(record.id.clone(), Some(record));
        self
    }

    /// Register an id that resolves to None (record absent).
    fn with_missing(mut self, id: &str) -> Self {
        self.records.insert(id.to_string(), None);
        self
    }

    fn with_error(mut self, error: ResolverError) -> Self {
        self.error = Some(error);
        self
    }
}

impl RecordResolver for FakeResolver {
    fn resolve(&self, id: &str) -> Result<Option<RetrievalRecord>, ResolverError> {
        if let Some(err) = &self.error {
            return Err(err.clone());
        }
        Ok(self.records.get(id).cloned().flatten())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_record(id: &str) -> RetrievalRecord {
    make_record_with_status(id, None)
}

fn make_record_with_status(id: &str, status: Option<&str>) -> RetrievalRecord {
    RetrievalRecord {
        id: id.to_string(),
        kind: "claim".to_string(),
        status: status.map(str::to_string),
        owner: None,
        verified_at: None,
        body: "Body.".to_string(),
        source: RetrievalSource {
            path: "docs/test.adoc".to_string(),
            line: 1,
            column: 1,
        },
        evidence: BTreeMap::new(),
        fields: BTreeMap::new(),
        relations: AgentJsonRelations::default(),
        search_match: None,
    }
}

fn make_record_with_expires(id: &str, expires_at: &str) -> RetrievalRecord {
    let mut record = make_record(id);
    record
        .fields
        .insert("expires_at".to_string(), expires_at.to_string());
    record
}

fn service(resolver: FakeResolver) -> ExplainService<FakeResolver, FakeClock> {
    ExplainService::new(
        resolver,
        FakeClock::zero(),
        std::path::PathBuf::from("docs.agent.json"),
    )
}

fn service_with_clock(
    resolver: FakeResolver,
    clock: FakeClock,
) -> ExplainService<FakeResolver, FakeClock> {
    ExplainService::new(
        resolver,
        clock,
        std::path::PathBuf::from("/tmp/adoc/docs.agent.json"),
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn execute_returns_view_for_existing_id() {
    let record = make_record("billing.credits");
    let svc = service(FakeResolver::new().with_record(record));

    let view = svc.execute("billing.credits").expect("view is returned");

    assert_eq!(view.record.id, "billing.credits");
}

#[test]
fn execute_returns_not_found_for_missing_id() {
    let svc = service(FakeResolver::new());

    let err = svc.execute("billing.missing").expect_err("not-found error");

    assert!(
        matches!(err, ExplainError::NotFound(ref id) if id == "billing.missing"),
        "unexpected error: {err}"
    );
}

#[test]
fn execute_populates_related_statuses_for_each_relation_target() {
    let primary = RetrievalRecord {
        id: "billing.credits".to_string(),
        kind: "claim".to_string(),
        status: Some("verified".to_string()),
        owner: None,
        verified_at: None,
        body: "Body.".to_string(),
        source: RetrievalSource {
            path: "docs/test.adoc".to_string(),
            line: 1,
            column: 1,
        },
        evidence: BTreeMap::new(),
        fields: BTreeMap::new(),
        relations: AgentJsonRelations {
            depends_on: vec!["a".to_string()],
            supersedes: vec!["b".to_string()],
            related_to: vec!["c".to_string()],
        },
        search_match: None,
    };

    let resolver = FakeResolver::new()
        .with_record(primary)
        .with_record(make_record_with_status("a", Some("verified")))
        .with_record(make_record_with_status("b", Some("contradicted")))
        .with_record(make_record_with_status("c", None));

    let svc = service(resolver);
    let view = svc.execute("billing.credits").expect("view is returned");

    let mut expected = BTreeMap::new();
    expected.insert("a".to_string(), Some("verified".to_string()));
    expected.insert("b".to_string(), Some("contradicted".to_string()));
    expected.insert("c".to_string(), None);

    assert_eq!(view.related_statuses, expected);
}

#[test]
fn execute_propagates_resolver_errors() {
    let resolver = FakeResolver::new().with_error(ResolverError::Io("disk gone".to_string()));
    let svc = service(resolver);

    let err = svc
        .execute("billing.credits")
        .expect_err("error propagated");

    assert!(
        matches!(err, ExplainError::Resolver(_)),
        "unexpected error: {err}"
    );
}

#[test]
fn execute_handles_missing_relation_targets_as_unknown() {
    let primary = RetrievalRecord {
        id: "billing.credits".to_string(),
        kind: "claim".to_string(),
        status: Some("verified".to_string()),
        owner: None,
        verified_at: None,
        body: "Body.".to_string(),
        source: RetrievalSource {
            path: "docs/test.adoc".to_string(),
            line: 1,
            column: 1,
        },
        evidence: BTreeMap::new(),
        fields: BTreeMap::new(),
        relations: AgentJsonRelations {
            depends_on: vec!["missing.target".to_string()],
            supersedes: vec![],
            related_to: vec![],
        },
        search_match: None,
    };

    let resolver = FakeResolver::new()
        .with_record(primary)
        .with_missing("missing.target");

    let svc = service(resolver);
    let view = svc
        .execute("billing.credits")
        .expect("view is returned even when target is missing");

    assert_eq!(
        view.related_statuses.get("missing.target"),
        Some(&None),
        "missing relation target maps to None, not an error"
    );
}

// ---------------------------------------------------------------------------
// Expires tests (slice 6)
// ---------------------------------------------------------------------------

#[test]
fn execute_populates_expires_when_field_is_parseable_iso_date() {
    let mut record = make_record("billing.credits");
    record
        .fields
        .insert("expires_at".to_string(), "2026-08-04".to_string());
    let svc = service(FakeResolver::new().with_record(record));

    let view = svc.execute("billing.credits").expect("view returned");

    assert_eq!(
        view.expires,
        Some(ExpiresInfo {
            date: NaiveDate::from_ymd_opt(2026, 8, 4).unwrap(),
            days_until: 88,
        }),
        "expires should be populated with correct date and days_until"
    );
}

#[test]
fn execute_leaves_expires_none_when_field_is_missing() {
    let record = make_record("billing.credits");
    let svc = service(FakeResolver::new().with_record(record));

    let view = svc.execute("billing.credits").expect("view returned");

    assert_eq!(
        view.expires, None,
        "expires should be None when field absent"
    );
}

#[test]
fn execute_leaves_expires_none_when_field_is_unparseable() {
    let mut record = make_record("billing.credits");
    record
        .fields
        .insert("expires_at".to_string(), "not-a-date".to_string());
    let svc = service(FakeResolver::new().with_record(record));

    let view = svc.execute("billing.credits").expect("view returned");

    assert_eq!(
        view.expires, None,
        "expires should be None when field is unparseable"
    );
}

#[test]
fn execute_handles_expired_dates_with_negative_days_until() {
    let mut record = make_record("billing.credits");
    // 2026-04-30 is 8 days before FakeClock today (2026-05-08)
    record
        .fields
        .insert("expires_at".to_string(), "2026-04-30".to_string());
    let svc = service(FakeResolver::new().with_record(record));

    let view = svc.execute("billing.credits").expect("view returned");

    let expires = view.expires.expect("expires should be populated");
    assert_eq!(expires.date, NaiveDate::from_ymd_opt(2026, 4, 30).unwrap());
    assert_eq!(
        expires.days_until, -8,
        "days_until should be negative for past date"
    );
}

#[test]
fn execute_populates_expires_with_zero_days_until_when_target_is_today() {
    // FakeClock::today() returns 2026-05-08; expires_at set to the same date.
    let record = make_record_with_expires("billing.credits", "2026-05-08");
    let svc = service(FakeResolver::new().with_record(record));

    let view = svc.execute("billing.credits").expect("view returned");

    let expires = view.expires.expect("expires should be Some");
    assert_eq!(expires.days_until, 0);
    assert_eq!(expires.date, NaiveDate::from_ymd_opt(2026, 5, 8).unwrap());
}

// ---------------------------------------------------------------------------
// RenderMeta tests (slice 8)
// ---------------------------------------------------------------------------

#[test]
fn render_meta_artifact_matches_service_artifact_path() {
    let record = make_record("billing.credits");
    let clock = FakeClock::zero();
    let svc = service_with_clock(FakeResolver::new().with_record(record), clock);

    let view = svc.execute("billing.credits").expect("view returned");

    assert_eq!(
        view.render_meta.artifact,
        std::path::PathBuf::from("/tmp/adoc/docs.agent.json"),
        "render_meta.artifact must equal the path passed to ExplainService::new"
    );
}

#[test]
fn render_meta_trust_is_some_when_fields_trust_present() {
    let mut record = make_record("billing.credits");
    record
        .fields
        .insert("trust".to_string(), "team".to_string());
    let clock = FakeClock::zero();
    let svc = service_with_clock(FakeResolver::new().with_record(record), clock);

    let view = svc.execute("billing.credits").expect("view returned");

    assert_eq!(
        view.render_meta.trust,
        Some("team".to_string()),
        "render_meta.trust must be Some(\"team\") when fields[\"trust\"] == \"team\""
    );
}

#[test]
fn render_meta_trust_is_none_when_fields_trust_absent() {
    let record = make_record("billing.credits");
    let clock = FakeClock::zero();
    let svc = service_with_clock(FakeResolver::new().with_record(record), clock);

    let view = svc.execute("billing.credits").expect("view returned");

    assert_eq!(
        view.render_meta.trust, None,
        "render_meta.trust must be None when fields[\"trust\"] is absent"
    );
}

#[test]
fn render_meta_duration_reflects_clock_delta() {
    // FakeClock::with_step(60ms) returns:
    //   1st call (started) = baseline + 0ms
    //   2nd call (ended)   = baseline + 60ms
    // So duration = 60ms.
    let record = make_record("billing.credits");
    let clock = FakeClock::with_step(Duration::from_millis(60));
    let svc = service_with_clock(FakeResolver::new().with_record(record), clock);

    let view = svc.execute("billing.credits").expect("view returned");

    assert_eq!(
        view.render_meta.duration,
        Duration::from_millis(60),
        "render_meta.duration must equal the delta between the two clock snapshots"
    );
}
