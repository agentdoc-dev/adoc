use std::collections::BTreeMap;

use adoc_core::{AgentJsonRelations, ExpiresInfo, RetrievalRecord, RetrievalSource};
use adoc_core::{Clock, ExplainError, ExplainService, RecordResolver, ResolverError};
use chrono::NaiveDate;
use std::time::Instant;

// ---------------------------------------------------------------------------
// Fakes
// ---------------------------------------------------------------------------

struct FakeClock;

impl Clock for FakeClock {
    fn today(&self) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 5, 8).expect("valid date")
    }

    fn now_instant(&self) -> Instant {
        Instant::now()
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

fn service(resolver: FakeResolver) -> ExplainService<FakeResolver, FakeClock> {
    ExplainService::new(
        resolver,
        FakeClock,
        std::path::PathBuf::from("docs.agent.json"),
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
