#![doc(html_root_url = "https://docs.rs/yolo-xml/0.0.1")]
#![warn(clippy::pedantic)]

use std::error::Error;

use futures_core::Stream;

#[cfg(doctest)]
pub mod readme {
	doc_comment::doctest!("../README.md");
}

pub struct XmlParser<'a, E: ?Sized> {
	input: &'a mut dyn Stream<Item = Result<char, Box<E>>>,
}
