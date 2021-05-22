# yolo-xml

[![Lib.rs](https://img.shields.io/badge/Lib.rs-*-84f)](https://lib.rs/crates/yolo-xml)
[![Crates.io](https://img.shields.io/crates/v/yolo-xml)](https://crates.io/crates/yolo-xml)
[![Docs.rs](https://docs.rs/yolo-xml/badge.svg)](https://docs.rs/yolo-xml)

![Rust 1.51](https://img.shields.io/static/v1?logo=Rust&label=&message=1.51&color=grey)
[![CI](https://github.com/Tamschi/yolo-xml/workflows/CI/badge.svg?branch=develop)](https://github.com/Tamschi/yolo-xml/actions?query=workflow%3ACI+branch%3Adevelop)
![Crates.io - License](https://img.shields.io/crates/l/yolo-xml/0.0.1)

[![GitHub](https://img.shields.io/static/v1?logo=GitHub&label=&message=%20&color=grey)](https://github.com/Tamschi/yolo-xml)
[![open issues](https://img.shields.io/github/issues-raw/Tamschi/yolo-xml)](https://github.com/Tamschi/yolo-xml/issues)
[![open pull requests](https://img.shields.io/github/issues-pr-raw/Tamschi/yolo-xml)](https://github.com/Tamschi/yolo-xml/pulls)
[![crev reviews](https://web.crev.dev/rust-reviews/badge/crev_count/yolo-xml.svg)](https://web.crev.dev/rust-reviews/crate/yolo-xml/)

An XML parser that respects your time.

`yolo-xml` aims to be an easy-to-use XML parsing library that is *strictly* compliant to the XML specification and *safe* to run against potentially malicious inputs.

> These go hand-in-hand; **once `yolo-xml` has been sufficiently audited**, you should be able to use `yolo-xml` as barrier against [invalid XML format confusion](https://siguza.github.io/psychicpaper/) attacks due to its strictness, for example.
>
> In an ideal world nearly all parsers would be strict of course, but sometimes that's just not an option for one reason or another. (It should probably be more common though.)

Apart from this, the library should be usable in as many ways as possible, for example with streamed XML as used in the XMPP protocol (which is the main motivation for creating `yolo-xml`).

## Installation

Please use [cargo-edit](https://crates.io/crates/cargo-edit) to always add the latest version of this library:

```cmd
cargo add yolo-xml
```

## Example

```rust
// TODO_EXAMPLE
```

## License

Licensed under either of

* Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
* MIT license
   ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

## [Code of Conduct](CODE_OF_CONDUCT.md)

## [Changelog](CHANGELOG.md)

## Versioning

`yolo-xml` strictly follows [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html) with the following exceptions:

* The minor version will not reset to 0 on major version changes (except for v1).  
Consider it the global feature level.
* The patch version will not reset to 0 on major or minor version changes (except for v0.1 and v1).  
Consider it the global patch level.

This includes the Rust version requirement specified above.  
Earlier Rust versions may be compatible, but this can change with minor or patch releases.

Which versions are affected by features and patches can be determined from the respective headings in [CHANGELOG.md](CHANGELOG.md).
