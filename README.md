# http-fresh

[![All Contributors](https://img.shields.io/badge/all_contributors-1-orange.svg?style=flat-square)](#contributors-)

[![crates.io](https://img.shields.io/crates/v/http-fresh.svg)](https://crates.io/crates/http-fresh)
[![docs.rs](https://docs.rs/http-fresh/badge.svg)](https://docs.rs/http-fresh)
[![CI](https://github.com/trananhtung/http-fresh/actions/workflows/ci.yml/badge.svg)](https://github.com/trananhtung/http-fresh/actions/workflows/ci.yml)
[![license](https://img.shields.io/crates/l/http-fresh.svg)](#license)

**HTTP response freshness (conditional `GET`) checking for Rust.**

Decide whether a cached response is still *fresh* for a request — i.e. whether your
server may answer **`304 Not Modified`** instead of resending the body. `http-fresh`
evaluates the `If-None-Match` / `If-Modified-Since` request headers against the
response's `ETag` / `Last-Modified`, honoring `Cache-Control: no-cache`.

It is a faithful Rust port of the widely-used [`fresh`](https://www.npmjs.com/package/fresh)
npm package (the logic behind Express's `req.fresh`), which has no Rust equivalent.

- **Zero dependencies**
- **`#![no_std]`** — works anywhere `core` does
- **Zero heap allocation** — no `alloc` required
- Differential-tested against the reference `fresh` implementation

## Install

```toml
[dependencies]
http-fresh = "0.1"
```

## Usage

```rust
use http_fresh::{fresh, Request, Response};

// ETag-based conditional GET.
let req = Request::new().if_none_match("\"686897696a7c876b7e\"");
let res = Response::new().etag("\"686897696a7c876b7e\"");
assert!(fresh(&req, &res)); // matching ETag → fresh → send 304

let res = Response::new().etag("\"a-new-etag\"");
assert!(!fresh(&req, &res)); // changed → stale → send the body

// Date-based conditional GET.
let req = Request::new().if_modified_since("Wed, 21 Oct 2015 07:28:00 GMT");
let res = Response::new().last_modified("Mon, 19 Oct 2015 07:28:00 GMT");
assert!(fresh(&req, &res)); // not modified since → fresh

// Cache-Control: no-cache always forces a revalidation.
let req = req.cache_control("no-cache");
assert!(!fresh(&req, &Response::new().last_modified("Mon, 19 Oct 2015 07:28:00 GMT")));
```

You can also build the headers as plain struct literals (all fields are `Option<&str>`
and default to `None`):

```rust
use http_fresh::{fresh, Request, Response};

let req = Request { if_none_match: Some("*"), ..Default::default() };
let res = Response { etag: Some("\"v1\""), ..Default::default() };
assert!(fresh(&req, &res));
```

## Semantics

`fresh` returns `true` (the response is fresh — send `304`) only when:

1. The request is **conditional** (has `If-None-Match` and/or `If-Modified-Since`); an
   unconditional request is never fresh.
2. `Cache-Control: no-cache` is **not** present (it always forces stale).
3. If `If-None-Match` is present and not `*`, the response `ETag` matches one of its
   entries, using weak/strong (`W/`) comparison.
4. If `If-Modified-Since` is present, the response `Last-Modified` is **no newer** than it.

Empty-string headers are treated as absent.

## Differences from the npm package

The npm package parses dates with JavaScript's `Date.parse`, which is lenient and — for
timezone-less formats such as `asctime` — interprets them in the **host machine's local
timezone**, so its result is not reproducible across machines. `http-fresh` instead parses
exactly the three date formats mandated by
[RFC 9110 §5.6.7](https://www.rfc-editor.org/rfc/rfc9110#section-5.6.7) — `IMF-fixdate`,
obsolete `RFC 850`, and `asctime` — always in **GMT**. Any string outside those formats is
treated as unparseable, which makes the response **stale** (a safe, revalidating default).
For real-world HTTP traffic, which uses `IMF-fixdate`, the behavior is identical.

## Contributors ✨

This project follows the [all-contributors](https://github.com/all-contributors/all-contributors) specification. Contributions of any kind are welcome — code, docs, bug reports, ideas, reviews! See the [emoji key](https://allcontributors.org/docs/en/emoji-key) for how each contribution is recognized, and open a PR or issue to get involved.

Thanks goes to these wonderful people:

<!-- ALL-CONTRIBUTORS-LIST:START - Do not remove or modify this section -->
<!-- prettier-ignore-start -->
<!-- markdownlint-disable -->
<table>
  <tbody>
    <tr>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/trananhtung"><img src="https://avatars.githubusercontent.com/u/30992229?v=4?s=100" width="100px;" alt="Tung Tran"/><br /><sub><b>Tung Tran</b></sub></a><br /><a href="https://github.com/trananhtung/./commits?author=trananhtung" title="Code">💻</a> <a href="#maintenance-trananhtung" title="Maintenance">🚧</a></td>
    </tr>
  </tbody>
</table>

<!-- markdownlint-restore -->
<!-- prettier-ignore-end -->

<!-- ALL-CONTRIBUTORS-LIST:END -->

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.
