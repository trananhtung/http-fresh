//! Integration tests exercising the public API of `http-fresh`.

use http_fresh::{fresh, Request, Response};

#[test]
fn typical_etag_conditional_get() {
    // Client revalidates with the ETag it has cached.
    let req = Request::new().if_none_match("\"686897696a7c876b7e\"");
    let res = Response::new().etag("\"686897696a7c876b7e\"");
    assert!(fresh(&req, &res), "matching ETag should be fresh (304)");

    let res = Response::new().etag("\"new-version\"");
    assert!(!fresh(&req, &res), "changed ETag should be stale (200)");
}

#[test]
fn typical_modified_since_conditional_get() {
    let req = Request::new().if_modified_since("Wed, 21 Oct 2015 07:28:00 GMT");
    // Resource unchanged since that date.
    let res = Response::new().last_modified("Mon, 19 Oct 2015 07:28:00 GMT");
    assert!(fresh(&req, &res));
    // Resource changed after that date.
    let res = Response::new().last_modified("Fri, 23 Oct 2015 07:28:00 GMT");
    assert!(!fresh(&req, &res));
}

#[test]
fn both_validators_must_pass() {
    let req = Request::new()
        .if_none_match("\"abc\"")
        .if_modified_since("Wed, 21 Oct 2015 07:28:00 GMT");
    // ETag matches but Last-Modified is newer → stale.
    let res = Response::new()
        .etag("\"abc\"")
        .last_modified("Thu, 22 Oct 2015 07:28:00 GMT");
    assert!(!fresh(&req, &res));
    // Both pass → fresh.
    let res = Response::new()
        .etag("\"abc\"")
        .last_modified("Tue, 20 Oct 2015 07:28:00 GMT");
    assert!(fresh(&req, &res));
}

#[test]
fn no_cache_overrides_a_match() {
    let req = Request::new()
        .if_none_match("\"abc\"")
        .cache_control("no-cache");
    let res = Response::new().etag("\"abc\"");
    assert!(!fresh(&req, &res));
}

#[test]
fn star_matches_when_any_representation_exists() {
    let req = Request::new().if_none_match("*");
    assert!(fresh(&req, &Response::new().etag("\"anything\"")));
    assert!(fresh(&req, &Response::new())); // `*` does not require an ETag
}

#[test]
fn direct_struct_construction_works() {
    let req = Request {
        if_none_match: Some("\"x\""),
        ..Default::default()
    };
    assert!(fresh(
        &req,
        &Response {
            etag: Some("\"x\""),
            ..Default::default()
        }
    ));
}
