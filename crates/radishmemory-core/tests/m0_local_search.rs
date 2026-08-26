use serde_json as _;
use sha2 as _;
use time as _;
use unicode_normalization as _;

use radishmemory_core::{
    CoreErrorCode, Identifier, InvalidCanonicalObjectReason, LocalSearchRequest, NonEmptyText,
    Sensitivity, Timestamp,
};

fn id(value: &str) -> Identifier {
    Identifier::new(value).expect("synthetic identifier must be valid")
}

fn text(value: &str) -> NonEmptyText {
    NonEmptyText::new(value).expect("synthetic text must be nonempty")
}

fn timestamp(value: &str) -> Timestamp {
    Timestamp::parse(value).expect("synthetic timestamp must be valid")
}

#[test]
fn local_search_request_requires_terms_top_k_and_an_explicit_sensitivity_scope() {
    for (query, top_k, sensitivities, expected_reason) in [
        (
            "   ",
            5,
            vec![Sensitivity::Personal],
            InvalidCanonicalObjectReason::EmptyText,
        ),
        (
            "synthetic query",
            0,
            vec![Sensitivity::Personal],
            InvalidCanonicalObjectReason::EmptyRequiredCollection,
        ),
        (
            "synthetic query",
            5,
            vec![],
            InvalidCanonicalObjectReason::EmptyRequiredCollection,
        ),
    ] {
        let error = LocalSearchRequest::new(
            id("namespace-1"),
            text(query),
            timestamp("2026-08-26T08:00:00Z"),
            top_k,
            sensitivities,
        )
        .expect_err("invalid search boundary must fail closed");
        assert_eq!(error.code(), CoreErrorCode::InvalidCanonicalObject);
        assert_eq!(
            error.invalid_canonical_object_reason(),
            Some(expected_reason)
        );
    }
}

#[test]
fn valid_search_request_retains_scope_without_debugging_query_content() {
    let private_query = "private-synthetic-query-marker";
    let request = LocalSearchRequest::new(
        id("namespace-1"),
        text(private_query),
        timestamp("2026-08-26T08:00:00Z"),
        5,
        [Sensitivity::Personal, Sensitivity::Sensitive],
    )
    .expect("valid local search request must construct");

    assert_eq!(request.namespace_id().as_str(), "namespace-1");
    assert_eq!(request.query().as_str(), private_query);
    assert_eq!(request.top_k(), 5);
    assert!(request.allows_sensitivity(Sensitivity::Personal));
    assert!(request.allows_sensitivity(Sensitivity::Sensitive));
    assert!(!request.allows_sensitivity(Sensitivity::Restricted));
    assert!(!format!("{request:?}").contains(private_query));
}
