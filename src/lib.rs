//! # http-fresh — HTTP response freshness (conditional GET) checking
//!
//! Decide whether a cached response is still *fresh* for a request, i.e. whether the
//! server may answer `304 Not Modified` instead of resending the body. This evaluates
//! the `If-None-Match` / `If-Modified-Since` request headers against the response's
//! `ETag` / `Last-Modified`, honoring `Cache-Control: no-cache`.
//!
//! A faithful Rust port of the [`fresh`](https://www.npmjs.com/package/fresh) npm
//! package (the logic behind Express's `req.fresh`). **Zero dependencies, `#![no_std]`,
//! and zero heap allocation.**
//!
//! ```
//! use http_fresh::{fresh, Request, Response};
//!
//! // ETag matches → response is fresh → caller can send 304.
//! let req = Request::new().if_none_match("\"abc\"");
//! let res = Response::new().etag("\"abc\"");
//! assert!(fresh(&req, &res));
//!
//! // A different ETag → stale → caller must send the body.
//! let res = Response::new().etag("\"xyz\"");
//! assert!(!fresh(&req, &res));
//!
//! // `Cache-Control: no-cache` always forces stale.
//! let req = req.cache_control("no-cache");
//! assert!(!fresh(&req, &Response::new().etag("\"abc\"")));
//! ```
//!
//! ## Differences from the npm package
//!
//! The npm package parses dates with JavaScript's `Date.parse`, which is lenient and —
//! for timezone-less formats such as `asctime` — interprets them in the host machine's
//! *local* timezone (so its result is not reproducible across machines). This crate
//! instead parses exactly the three date formats mandated by
//! [RFC 9110 §5.6.7](https://www.rfc-editor.org/rfc/rfc9110#section-5.6.7)
//! (`IMF-fixdate`, `RFC 850`, and `asctime`), always in **GMT**. Any string outside
//! those formats is treated as unparseable, which makes the response **stale** (a safe,
//! revalidating default). For real-world HTTP traffic — which uses `IMF-fixdate` — the
//! behavior is identical to the npm package.

#![no_std]
#![forbid(unsafe_code)]
#![doc(html_root_url = "https://docs.rs/http-fresh/0.1.0")]

// Compile-test the README's examples as part of `cargo test`.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

/// The request-side headers that drive a freshness check.
///
/// Build one with [`Request::new`] and the chainable setters, or construct it directly
/// (all fields are public). A field left `None` — or set to an empty string — is treated
/// as absent, mirroring the npm package's falsy-header handling.
///
/// ```
/// use http_fresh::Request;
/// let req = Request::new()
///     .if_none_match("\"v1\"")
///     .cache_control("max-age=0");
/// assert_eq!(req.if_none_match, Some("\"v1\""));
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Request<'a> {
    /// The `If-None-Match` header value (an `ETag` list or `*`).
    pub if_none_match: Option<&'a str>,
    /// The `If-Modified-Since` header value (an HTTP date).
    pub if_modified_since: Option<&'a str>,
    /// The `Cache-Control` header value.
    pub cache_control: Option<&'a str>,
}

impl<'a> Request<'a> {
    /// Create an empty [`Request`] with all headers absent.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the `If-None-Match` header.
    #[must_use]
    pub fn if_none_match(mut self, value: &'a str) -> Self {
        self.if_none_match = Some(value);
        self
    }

    /// Set the `If-Modified-Since` header.
    #[must_use]
    pub fn if_modified_since(mut self, value: &'a str) -> Self {
        self.if_modified_since = Some(value);
        self
    }

    /// Set the `Cache-Control` header.
    #[must_use]
    pub fn cache_control(mut self, value: &'a str) -> Self {
        self.cache_control = Some(value);
        self
    }
}

/// The response-side validators that a fresh request must match.
///
/// Build one with [`Response::new`] and the chainable setters, or construct it directly.
/// A field left `None` — or set to an empty string — is treated as absent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Response<'a> {
    /// The response `ETag` header value.
    pub etag: Option<&'a str>,
    /// The response `Last-Modified` header value (an HTTP date).
    pub last_modified: Option<&'a str>,
}

impl<'a> Response<'a> {
    /// Create an empty [`Response`] with all validators absent.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the `ETag` header.
    #[must_use]
    pub fn etag(mut self, value: &'a str) -> Self {
        self.etag = Some(value);
        self
    }

    /// Set the `Last-Modified` header.
    #[must_use]
    pub fn last_modified(mut self, value: &'a str) -> Self {
        self.last_modified = Some(value);
        self
    }
}

/// Returns `true` if the response is **fresh** for this request — meaning the server may
/// answer `304 Not Modified` rather than resending the body.
///
/// The check mirrors the npm `fresh` package:
///
/// 1. An *unconditional* request (no `If-None-Match` and no `If-Modified-Since`) is never
///    fresh.
/// 2. `Cache-Control: no-cache` always forces stale.
/// 3. If `If-None-Match` is present and not `*`, the response `ETag` must match one of its
///    entries (with weak/strong `W/` comparison).
/// 4. If `If-Modified-Since` is present, the response `Last-Modified` must be no newer than
///    it.
///
/// Empty-string headers are treated as absent.
///
/// ```
/// use http_fresh::{fresh, Request, Response};
///
/// let req = Request::new().if_modified_since("Sun, 06 Nov 1994 08:49:37 GMT");
/// let res = Response::new().last_modified("Sat, 05 Nov 1994 08:49:37 GMT");
/// assert!(fresh(&req, &res)); // not modified since → fresh
/// ```
#[must_use]
pub fn fresh(req: &Request<'_>, res: &Response<'_>) -> bool {
    let modified_since = present(req.if_modified_since);
    let none_match = present(req.if_none_match);

    // Unconditional request.
    if modified_since.is_none() && none_match.is_none() {
        return false;
    }

    // `Cache-Control: no-cache` forces an end-to-end reload.
    if let Some(cc) = present(req.cache_control) {
        if has_no_cache(cc) {
            return false;
        }
    }

    // If-None-Match.
    if let Some(none_match) = none_match {
        if none_match != "*" {
            let Some(etag) = present(res.etag) else {
                return false;
            };
            if !none_match
                .split(',')
                .any(|tok| etag_matches(trim_spaces(tok), etag))
            {
                return false;
            }
        }
    }

    // If-Modified-Since.
    if let Some(modified_since) = modified_since {
        let Some(last_modified) = present(res.last_modified) else {
            return false;
        };
        let modified_stale = match (
            parse_http_date(last_modified),
            parse_http_date(modified_since),
        ) {
            (Some(last), Some(since)) => last > since,
            // An unparseable date compares like `NaN` in JS: never `<=`, so stale.
            _ => true,
        };
        if modified_stale {
            return false;
        }
    }

    true
}

/// Map a `None`/empty header to absent, mirroring JavaScript falsy-string handling.
fn present(value: Option<&str>) -> Option<&str> {
    value.filter(|v| !v.is_empty())
}

/// Trim leading and trailing ASCII spaces (`0x20` only — not tabs), matching the
/// reference's `parseTokenList`.
fn trim_spaces(s: &str) -> &str {
    s.trim_matches(' ')
}

/// Whether `tag` from an `If-None-Match` list matches `etag`, with weak/strong comparison:
/// `tag == etag`, `tag == "W/" + etag`, or `"W/" + tag == etag`.
fn etag_matches(tag: &str, etag: &str) -> bool {
    tag == etag || tag.strip_prefix("W/") == Some(etag) || etag.strip_prefix("W/") == Some(tag)
}

/// Replicates `/(?:^|,)\s*?no-cache\s*?(?:,|$)/` (case-sensitive) without a regex engine.
///
/// A `no-cache` token qualifies when it is bounded — on each side, after skipping any
/// JavaScript-`\s` whitespace — by a comma or the string boundary.
fn has_no_cache(cc: &str) -> bool {
    let needle = "no-cache";
    let mut search_from = 0;
    while let Some(rel) = cc[search_from..].find(needle) {
        let start = search_from + rel;
        let end = start + needle.len();

        let left_ok = {
            // Walk left over whitespace; reached the start, or the bounding char is ','.
            let mut iter = cc[..start]
                .chars()
                .rev()
                .skip_while(|&c| is_js_whitespace(c));
            matches!(iter.next(), None | Some(','))
        };
        let right_ok = {
            // Walk right over whitespace; reached the end, or the bounding char is ','.
            let mut iter = cc[end..].chars().skip_while(|&c| is_js_whitespace(c));
            matches!(iter.next(), None | Some(','))
        };
        if left_ok && right_ok {
            return true;
        }
        search_from = start + 1;
    }
    false
}

/// The set of characters matched by JavaScript's `\s` (without the `u` flag).
fn is_js_whitespace(c: char) -> bool {
    matches!(
        c,
        '\u{0009}'
            | '\u{000A}'
            | '\u{000B}'
            | '\u{000C}'
            | '\u{000D}'
            | '\u{0020}'
            | '\u{00A0}'
            | '\u{1680}'
            | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

/// Parse an HTTP date (RFC 9110 §5.6.7) into Unix epoch seconds, in GMT.
///
/// Accepts the three mandated formats: `IMF-fixdate`, obsolete `RFC 850`, and `asctime`.
/// Returns `None` for any other string.
fn parse_http_date(s: &str) -> Option<i64> {
    parse_imf(s)
        .or_else(|| parse_rfc850(s))
        .or_else(|| parse_asctime(s))
}

/// `Sun, 06 Nov 1994 08:49:37 GMT`
fn parse_imf(s: &str) -> Option<i64> {
    let comma = s.find(',')?;
    let rest = s[comma + 1..].trim_start_matches(' ');
    let mut it = rest.split(' ').filter(|p| !p.is_empty());
    let day = parse_fixed_digits(it.next()?, 2)?;
    let month = parse_month(it.next()?)?;
    let year_str = it.next()?;
    if year_str.len() != 4 {
        return None;
    }
    let year = parse_fixed_digits(year_str, 4)?;
    let (h, m, sec) = parse_time(it.next()?)?;
    if !it.next()?.eq_ignore_ascii_case("GMT") || it.next().is_some() {
        return None;
    }
    make_epoch(i64::from(year), month, i64::from(day), h, m, sec)
}

/// `Sunday, 06-Nov-94 08:49:37 GMT`
fn parse_rfc850(s: &str) -> Option<i64> {
    let comma = s.find(',')?;
    let rest = s[comma + 1..].trim_start_matches(' ');
    let mut it = rest.split(' ').filter(|p| !p.is_empty());
    let date_part = it.next()?;
    let (h, m, sec) = parse_time(it.next()?)?;
    if !it.next()?.eq_ignore_ascii_case("GMT") || it.next().is_some() {
        return None;
    }
    let mut dit = date_part.split('-');
    let day = parse_fixed_digits(dit.next()?, 2)?;
    let month = parse_month(dit.next()?)?;
    let yy_str = dit.next()?;
    if dit.next().is_some() || yy_str.len() != 2 {
        return None;
    }
    let yy = i64::from(parse_fixed_digits(yy_str, 2)?);
    // V8 pivot: 00–49 → 2000–2049, 50–99 → 1950–1999.
    let year = if yy < 50 { 2000 + yy } else { 1900 + yy };
    make_epoch(year, month, i64::from(day), h, m, sec)
}

/// `Sun Nov  6 08:49:37 1994` (day is space-padded)
fn parse_asctime(s: &str) -> Option<i64> {
    if s.contains(',') {
        return None;
    }
    let mut it = s.split(' ').filter(|p| !p.is_empty());
    let _weekday = it.next()?;
    let month = parse_month(it.next()?)?;
    let day = parse_day_1_or_2(it.next()?)?;
    let (h, m, sec) = parse_time(it.next()?)?;
    let year_str = it.next()?;
    if it.next().is_some() || year_str.len() != 4 {
        return None;
    }
    let year = i64::from(parse_fixed_digits(year_str, 4)?);
    make_epoch(year, month, i64::from(day), h, m, sec)
}

/// Parse `HH:MM:SS` into `(h, m, s)`.
fn parse_time(s: &str) -> Option<(i64, i64, i64)> {
    let mut it = s.split(':');
    let h = parse_fixed_digits(it.next()?, 2)?;
    let m = parse_fixed_digits(it.next()?, 2)?;
    let sec = parse_fixed_digits(it.next()?, 2)?;
    if it.next().is_some() || h > 23 || m > 59 || sec > 60 {
        return None;
    }
    Some((i64::from(h), i64::from(m), i64::from(sec)))
}

/// Parse exactly `width` ASCII digits into a `u32`.
fn parse_fixed_digits(s: &str, width: usize) -> Option<u32> {
    if s.len() != width || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// Parse a 1- or 2-digit day (for `asctime`, where single days are space-padded so the
/// field arrives as `"6"` after whitespace splitting).
fn parse_day_1_or_2(s: &str) -> Option<u32> {
    if s.is_empty() || s.len() > 2 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// Map a 3-letter English month abbreviation (case-insensitive) to `1..=12`.
fn parse_month(s: &str) -> Option<i64> {
    if s.len() != 3 {
        return None;
    }
    let mut buf = [0u8; 3];
    for (i, b) in s.bytes().enumerate() {
        buf[i] = b.to_ascii_lowercase();
    }
    match &buf {
        b"jan" => Some(1),
        b"feb" => Some(2),
        b"mar" => Some(3),
        b"apr" => Some(4),
        b"may" => Some(5),
        b"jun" => Some(6),
        b"jul" => Some(7),
        b"aug" => Some(8),
        b"sep" => Some(9),
        b"oct" => Some(10),
        b"nov" => Some(11),
        b"dec" => Some(12),
        _ => None,
    }
}

/// Convert a GMT calendar date/time to Unix epoch seconds.
fn make_epoch(year: i64, month: i64, day: i64, h: i64, m: i64, s: i64) -> Option<i64> {
    if !(1..=31).contains(&day) {
        return None;
    }
    let days = days_from_civil(year, month, day);
    Some(days * 86_400 + h * 3_600 + m * 60 + s)
}

/// Days since 1970-01-01 for a proleptic-Gregorian date (Howard Hinnant's algorithm).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let mp = if month > 2 { month - 3 } else { month + 9 }; // Mar=0..Feb=11
    let doy = (153 * mp + 2) / 5 + day - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unconditional_request_is_stale() {
        assert!(!fresh(&Request::new(), &Response::new()));
    }

    #[test]
    fn star_if_none_match_is_fresh() {
        let req = Request::new().if_none_match("*");
        assert!(fresh(&req, &Response::new()));
    }

    #[test]
    fn etag_strong_and_weak_matching() {
        let req = Request::new().if_none_match("\"foo\"");
        assert!(fresh(&req, &Response::new().etag("\"foo\"")));
        assert!(!fresh(&req, &Response::new().etag("\"bar\"")));
        // weak request tag vs strong response tag and vice versa
        assert!(fresh(
            &Request::new().if_none_match("W/\"foo\""),
            &Response::new().etag("\"foo\"")
        ));
        assert!(fresh(&req, &Response::new().etag("W/\"foo\"")));
    }

    #[test]
    fn etag_list_matches_any() {
        let req = Request::new().if_none_match("\"a\" , \"b\", \"c\"");
        assert!(fresh(&req, &Response::new().etag("\"b\"")));
        assert!(!fresh(&req, &Response::new().etag("\"z\"")));
    }

    #[test]
    fn missing_response_etag_is_stale() {
        let req = Request::new().if_none_match("\"foo\"");
        assert!(!fresh(&req, &Response::new()));
    }

    #[test]
    fn no_cache_forces_stale() {
        let res = Response::new().etag("\"foo\"");
        assert!(!fresh(
            &Request::new()
                .if_none_match("\"foo\"")
                .cache_control("no-cache"),
            &res
        ));
        assert!(!fresh(
            &Request::new()
                .if_none_match("\"foo\"")
                .cache_control("max-age=0, no-cache"),
            &res
        ));
        // not a bounded no-cache token → ignored
        assert!(fresh(
            &Request::new()
                .if_none_match("\"foo\"")
                .cache_control("no-cachex"),
            &res
        ));
        assert!(fresh(
            &Request::new()
                .if_none_match("\"foo\"")
                .cache_control("public, max-age=0"),
            &res
        ));
    }

    #[test]
    fn modified_since_dates() {
        let res = Response::new().last_modified("Sun, 06 Nov 1994 08:49:37 GMT");
        // last-modified == if-modified-since → fresh
        assert!(fresh(
            &Request::new().if_modified_since("Sun, 06 Nov 1994 08:49:37 GMT"),
            &res
        ));
        // last-modified older → fresh
        assert!(fresh(
            &Request::new().if_modified_since("Mon, 07 Nov 1994 08:49:37 GMT"),
            &res
        ));
        // last-modified newer → stale
        assert!(!fresh(
            &Request::new().if_modified_since("Sat, 05 Nov 1994 08:49:37 GMT"),
            &res
        ));
    }

    #[test]
    fn missing_last_modified_is_stale() {
        let req = Request::new().if_modified_since("Sun, 06 Nov 1994 08:49:37 GMT");
        assert!(!fresh(&req, &Response::new()));
    }

    #[test]
    fn unparseable_date_is_stale() {
        let req = Request::new().if_modified_since("not a date");
        let res = Response::new().last_modified("Sun, 06 Nov 1994 08:49:37 GMT");
        assert!(!fresh(&req, &res));
    }

    #[test]
    fn empty_headers_treated_as_absent() {
        assert!(!fresh(
            &Request::new().if_modified_since("").if_none_match(""),
            &Response::new()
        ));
    }

    #[test]
    fn all_three_date_formats_parse_equal() {
        let imf = parse_http_date("Sun, 06 Nov 1994 08:49:37 GMT").unwrap();
        let rfc850 = parse_http_date("Sunday, 06-Nov-94 08:49:37 GMT").unwrap();
        let asctime = parse_http_date("Sun Nov  6 08:49:37 1994").unwrap();
        assert_eq!(imf, rfc850);
        assert_eq!(imf, asctime);
        assert_eq!(imf, 784_111_777);
    }

    #[test]
    fn rfc850_two_digit_year_pivot() {
        let y49 = parse_http_date("Sun, 06-Nov-49 00:00:00 GMT").unwrap();
        let y50 = parse_http_date("Sun, 06-Nov-50 00:00:00 GMT").unwrap();
        assert!(y49 > y50); // 2049 vs 1950
    }
}
