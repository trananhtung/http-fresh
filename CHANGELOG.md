# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0]

### Added

- Initial release: `fresh(&Request, &Response) -> bool` for HTTP conditional-`GET`
  freshness checking — a faithful, zero-dependency, `no_std`, zero-alloc port of the
  `fresh` npm package.
- `If-None-Match` (with weak/strong `W/` ETag comparison and `*`), `If-Modified-Since`
  vs `Last-Modified`, and `Cache-Control: no-cache` handling.
- HTTP-date parsing for the three RFC 9110 §5.6.7 formats (`IMF-fixdate`, `RFC 850`,
  `asctime`), always in GMT.
