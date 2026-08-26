use std::error::Error;

// Cargo exposes package dependencies to each integration-test crate; these
// imports keep the workspace's strict unused-dependency lint meaningful there.
use sha2 as _;
use time as _;
use unicode_normalization as _;

use radishmemory_core::{
    CoreErrorCode, InvalidTimeReason, NonCanonicalJsonReason, TimePrecision, Timestamp, ValidTime,
    ValidTimeMode, canonicalize_json, compute_digest, verify_digest,
};

#[test]
fn frozen_digest_vectors_match() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/m0/local-memory-loop.v1.json"
    ))
    .expect("frozen fixture must be valid JSON");
    let vectors = fixture["digest_vectors"]
        .as_array()
        .expect("digest_vectors must be an array");

    for vector in vectors {
        let profile = vector["profile"]
            .as_str()
            .expect("fixture profile must be a string");
        let expected = vector["expected_sha256"]
            .as_str()
            .expect("fixture digest must be a string");
        let input = match profile {
            "exact-bytes-v1" | "utf8-nfc-text-v1" => vector["input_text"]
                .as_str()
                .expect("text vector must contain input_text")
                .to_owned(),
            "canonical-json-v1" => {
                serde_json::to_string(&vector["input_value"]).expect("JSON vector must serialize")
            }
            profile => panic!("unexpected frozen digest profile: {profile}"),
        };

        let digest = compute_digest(profile, &input).expect("frozen vector must compute");
        assert_eq!(digest.algorithm(), "sha256");
        assert_eq!(digest.profile().as_str(), profile);
        assert_eq!(digest.value(), expected);
    }
}

#[test]
fn frozen_suite_digest_matches_full_canonical_mapping() {
    let mut fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/m0/local-memory-loop.v1.json"
    ))
    .expect("frozen fixture must be valid JSON");
    let expected = fixture["suite_digest"]["value"]
        .as_str()
        .expect("suite digest must contain a value")
        .to_owned();
    fixture
        .as_object_mut()
        .expect("fixture root must be an object")
        .remove("suite_digest")
        .expect("suite digest must be present");
    let input = serde_json::to_string(&fixture).expect("fixture must serialize");

    let actual =
        compute_digest("canonical-json-v1", &input).expect("full frozen fixture must canonicalize");
    assert_eq!(actual.value(), expected);
}

#[test]
fn digest_profiles_fail_closed_and_report_mismatch() {
    let unsupported = compute_digest("future-profile", "synthetic input")
        .expect_err("unknown profiles must fail closed");
    assert_eq!(unsupported.code(), CoreErrorCode::UnsupportedProfile);

    let mismatch = verify_digest(
        "exact-bytes-v1",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "synthetic input",
    )
    .expect_err("incorrect digests must fail");
    assert_eq!(mismatch.code(), CoreErrorCode::DigestMismatch);
}

#[test]
fn nfc_profile_normalizes_only_semantic_text() {
    let decomposed =
        compute_digest("utf8-nfc-text-v1", "Cafe\u{301}").expect("NFC digest must compute");
    let composed = compute_digest("utf8-nfc-text-v1", "Café").expect("NFC digest must compute");
    assert_eq!(decomposed, composed);

    let exact_decomposed =
        compute_digest("exact-bytes-v1", "Cafe\u{301}").expect("exact digest must compute");
    let exact_composed =
        compute_digest("exact-bytes-v1", "Café").expect("exact digest must compute");
    assert_ne!(exact_decomposed, exact_composed);
}

#[test]
fn canonical_json_orders_keys_and_preserves_array_order() {
    let canonical = canonicalize_json(r#" {"雪":3,"a":[3,2,1],"\u0062":2,"\ud800\udc00":4} "#)
        .expect("valid JSON must canonicalize");
    assert_eq!(
        String::from_utf8(canonical).expect("canonical output must be UTF-8"),
        r#"{"a":[3,2,1],"b":2,"雪":3,"𐀀":4}"#
    );
}

#[test]
fn canonical_json_uses_required_escapes_and_direct_unicode() {
    let canonical = canonicalize_json(r#"{"value":"\u0061\n雪/\t\\\""}"#)
        .expect("valid string must canonicalize");
    assert_eq!(
        String::from_utf8(canonical).expect("canonical output must be UTF-8"),
        "{\"value\":\"a\\n雪/\\t\\\\\\\"\"}"
    );
}

#[test]
fn canonical_json_normalizes_integer_and_fraction_boundaries() {
    let cases = [
        ("-0", "0"),
        ("-0.000e+10", "0"),
        ("1.2300", "1.23"),
        ("1e3", "1000"),
        ("1e-3", "0.001"),
        ("1e+0", "1"),
        ("-1.20e+2", "-120"),
        ("0.0001000e2", "0.01"),
        ("1200e-2", "12"),
        ("100.00100", "100.001"),
        (
            "123456789012345678901234567890",
            "123456789012345678901234567890",
        ),
    ];
    for (input, expected) in cases {
        let actual = canonicalize_json(input).expect("valid number must canonicalize");
        assert_eq!(actual, expected.as_bytes(), "input: {input}");
    }
}

#[test]
fn canonical_json_rejects_duplicate_decoded_keys_and_null() {
    let duplicate = canonicalize_json(r#"{"a":1,"\u0061":2}"#)
        .expect_err("equivalent decoded keys must be duplicates");
    assert_eq!(duplicate.code(), CoreErrorCode::NonCanonicalJson);
    assert_eq!(
        duplicate.canonical_json_reason(),
        Some(NonCanonicalJsonReason::DuplicateKey)
    );

    let null = canonicalize_json(r#"{"nested":[null]}"#)
        .expect_err("M0 input must reject null before canonicalization");
    assert_eq!(
        null.canonical_json_reason(),
        Some(NonCanonicalJsonReason::NullForbidden)
    );
}

#[test]
fn canonical_json_rejects_invalid_syntax_and_preserves_parser_cause() {
    let invalid =
        canonicalize_json(r#"{"value":"\q"}"#).expect_err("invalid string escape must fail");
    assert_eq!(invalid.code(), CoreErrorCode::NonCanonicalJson);
    assert_eq!(
        invalid.canonical_json_reason(),
        Some(NonCanonicalJsonReason::Syntax)
    );
    assert!(invalid.source().is_some());

    let invalid_literal = canonicalize_json("null-private")
        .expect_err("a token beginning with null is still invalid syntax");
    assert_eq!(
        invalid_literal.canonical_json_reason(),
        Some(NonCanonicalJsonReason::Syntax)
    );

    let expansion = canonicalize_json("1e2000000")
        .expect_err("pathological ordinary-decimal expansion must be bounded");
    assert_eq!(
        expansion.canonical_json_reason(),
        Some(NonCanonicalJsonReason::NumberExpansionLimit)
    );
}

#[test]
fn timestamps_compare_in_utc_and_retain_external_precision() {
    let offset =
        Timestamp::parse("2026-08-23T10:11:12.340+08:00").expect("offset timestamp must parse");
    let utc = Timestamp::parse("2026-08-23T02:11:12.340Z").expect("UTC timestamp must parse");

    assert_eq!(offset, utc);
    assert_eq!(offset.original(), "2026-08-23T10:11:12.340+08:00");
    assert_eq!(offset.precision().fractional_second_digits(), 3);
    assert_eq!(offset.offset_seconds(), 8 * 60 * 60);
    assert_eq!(utc.offset_seconds(), 0);

    let high_precision_earlier = Timestamp::parse("2026-08-23T02:11:12.3400000001Z")
        .expect("arbitrary RFC 3339 fractional precision must parse");
    let high_precision_later = Timestamp::parse("2026-08-23T02:11:12.3400000002Z")
        .expect("arbitrary RFC 3339 fractional precision must parse");
    assert!(high_precision_earlier < high_precision_later);
    assert_eq!(
        high_precision_earlier
            .precision()
            .fractional_second_digits(),
        10
    );

    let same_with_trailing_zeroes = Timestamp::parse("2026-08-23T02:11:12.3400Z")
        .expect("equivalent fractional precision must parse");
    assert_eq!(utc, same_with_trailing_zeroes);

    let before_leap = Timestamp::parse("2016-12-31T23:59:59.9999999999Z")
        .expect("instant before leap second must parse");
    let leap = Timestamp::parse("2016-12-31T23:59:60.5Z").expect("RFC 3339 leap second must parse");
    let after_leap =
        Timestamp::parse("2017-01-01T00:00:00Z").expect("instant after leap second must parse");
    assert!(before_leap < leap);
    assert!(leap < after_leap);
}

#[test]
fn invalid_timestamp_has_stable_code_without_input_content() {
    let rejected = "not-a-time-private-content";
    let error = Timestamp::parse(rejected).expect_err("invalid RFC 3339 must fail");
    assert_eq!(error.code(), CoreErrorCode::InvalidTime);
    assert_eq!(error.invalid_time_reason(), Some(InvalidTimeReason::Parse));
    assert!(error.source().is_some());
    assert!(!error.to_string().contains(rejected));
    assert!(!format!("{error:?}").contains(rejected));
}

#[test]
fn valid_time_enforces_modes_and_half_open_intervals() {
    let start = Timestamp::parse("2026-08-23T00:00:00Z").expect("start must parse");
    let inside = Timestamp::parse("2026-08-23T12:00:00Z").expect("inside must parse");
    let end = Timestamp::parse("2026-08-24T00:00:00Z").expect("end must parse");
    let interval = ValidTime::new(
        ValidTimeMode::Interval,
        Some(start.clone()),
        Some(end.clone()),
        TimePrecision::Exact,
    )
    .expect("ordered interval must be valid");

    assert!(interval.contains(&start));
    assert!(interval.contains(&inside));
    assert!(!interval.contains(&end));

    let instant = ValidTime::new(
        ValidTimeMode::Instant,
        Some(inside.clone()),
        None,
        TimePrecision::Exact,
    )
    .expect("instant mode has only a start");
    assert!(instant.contains(&inside));
    assert!(!instant.contains(&start));

    let open_ended = ValidTime::new(
        ValidTimeMode::OpenEnded,
        Some(start.clone()),
        None,
        TimePrecision::Day,
    )
    .expect("open-ended mode has only a start");
    assert!(open_ended.contains(&inside));
    assert_eq!(open_ended.precision(), TimePrecision::Day);

    let unknown = ValidTime::new(ValidTimeMode::Unknown, None, None, TimePrecision::Unknown)
        .expect("unknown mode has no boundaries");
    assert!(!unknown.contains(&inside));

    let wrong_boundaries =
        ValidTime::new(ValidTimeMode::OpenEnded, None, None, TimePrecision::Exact)
            .expect_err("open-ended mode requires a start");
    assert_eq!(
        wrong_boundaries.invalid_time_reason(),
        Some(InvalidTimeReason::BoundaryCombination)
    );

    let reversed = ValidTime::new(
        ValidTimeMode::Interval,
        Some(end),
        Some(start),
        TimePrecision::Exact,
    )
    .expect_err("reversed interval must fail");
    assert_eq!(
        reversed.invalid_time_reason(),
        Some(InvalidTimeReason::IntervalOrder)
    );
}
