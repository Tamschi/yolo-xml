//! An XML parser that respects your time.
//!
//! [![Zulip Chat](https://img.shields.io/endpoint?label=chat&url=https%3A%2F%2Fiteration-square-automation.schichler.dev%2F.netlify%2Ffunctions%2Fstream_subscribers_shield%3Fstream%3Dproject%252Fyolo-xml)](https://iteration-square.schichler.dev/#narrow/stream/project.2Fyolo-xml)

#![doc(html_root_url = "https://docs.rs/yolo-xml/0.0.1")]
#![warn(clippy::pedantic, missing_docs)]
#![allow(
	clippy::semicolon_if_nothing_returned,
	clippy::if_not_else,
	clippy::single_match_else
)]
#![allow(missing_docs)]

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
mod readme {}

pub mod buffer;
pub mod scanner;
