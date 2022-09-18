//! An XML parser that respects your time.
//!
//! [![Zulip Chat](https://img.shields.io/endpoint?label=chat&url=https%3A%2F%2Fiteration-square-automation.schichler.dev%2F.netlify%2Ffunctions%2Fstream_subscribers_shield%3Fstream%3Dproject%252Fyolo-xml)](https://iteration-square.schichler.dev/#narrow/stream/project.2Fyolo-xml)

#![doc(html_root_url = "https://docs.rs/yolo-xml/0.0.1")]
#![no_std]
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

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

use core::{
	fmt::{self, Debug, Display, Formatter},
	future::Future,
	marker::PhantomData,
	mem::size_of_val,
	pin::Pin,
	result::Result,
};
use futures_core::Stream;
use iter::AsyncIterator;
use peek_stream::PeekStream;
use tap::Pipe as _;

mod iter;

/// Peek buffer size for the longest token I need to branch on.
///
/// That's probably `<!--`.
const PEEK: usize = 4;

pub mod blocking;
mod peek_stream;
pub mod predicate;

mod xml_element;
mod xml_name;

pub use xml_element::{XmlElement, XmlElementChildren};

pub struct Error;

impl Debug for Error {
	fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
		Display::fmt(self, f)
	}
}
impl Display for Error {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		f.write_str("Error deserializing XML. Check `tracing` logs for more information!")
	}
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

// Grammar definitions are referenced by comments of the form `// [n] name`.

pub enum Mode {
	Xml1_0,
	Xml1_1,
}

fn extend_zst_reference_mut<'a, T: ?Sized>(reference: &mut T) -> &'a mut T {
	assert_eq!(size_of_val::<T>(reference), 0);
	unsafe {
		// Safety: This is ZST reference, so the referenced memory is never accessed.
		// The associated vtable is static, if extant… at least as far as this library goes.
		&mut *(reference as *mut _)
	}
}

fn fake_discard_callback<'a, T, E>() -> &'a mut dyn FnMut(T) -> Result<(), E> {
	let mut discard = |_| Ok(());
	let discard: &mut dyn FnMut(T) -> Result<(), E> = &mut discard;
	extend_zst_reference_mut(discard)
}

/// [3] S
async fn skip_whitespace<
	'a,
	Input: AsyncIterator<Item<'a> = Result<char, E>>,
	E: Clone,
	const CAPACITY: usize,
>(
	mut input: Pin<&mut PeekStream<'_, Input, CAPACITY>>,
) -> Result<(), E> {
	while input
		.as_mut()
		.next_if(|next: &Result<char, _>| match next {
			Ok(char) => "\u{20}\u{9}\u{D}\u{A}".contains(*char),
			Err(_) => true,
		})
		.await
		.transpose()?
		.is_some()
	{}
	Ok(())
}

// pub type UnicodeNormalizer<'a, E> = dyn MutLenseMut<
// 	'a,
// 	dyn 'a + Stream<Item = Result<char, E>>,
// 	Output = dyn 'a + Stream<Item = Result<char, E>>,
// >;

struct NoNormalization;
// impl<'a, E: 'a> MutLenseMut<'a, dyn 'a + Stream<Item = Result<char, E>>> for NoNormalization {
// 	type Output = dyn 'a + Stream<Item = Result<char, E>>;

// 	fn lense<'b>(
// 		&'a mut self,
// 		args: &'b mut (dyn 'a + Stream<Item = Result<char, E>>),
// 	) -> &'b mut Self::Output {
// 		let _ = self;
// 		args
// 	}
// }

pub struct XmlParserOptions<'a, E: 'a> {
	on_mode: &'a mut dyn FnMut(Mode) -> Result<(), E>,
	// unicode_normalizer: &'a mut UnicodeNormalizer<'a, E>,
}
impl<'a, E: 'a> Default for XmlParserOptions<'a, E> {
	fn default() -> Self {
		Self {
			on_mode: fake_discard_callback(),
			// unicode_normalizer: extend_zst_reference_mut::<NoNormalization>(&mut NoNormalization),
		}
	}
}

pub struct XmlParser<'a, E> {
	input: &'a mut dyn Stream<Item = Result<char, E>>,
	mode: Mode,
	phantom: PhantomData<E>,
}
impl<'a, E> XmlParser<'a, E> {
	pub fn start(
		input: &'a mut dyn Stream<Item = Result<char, E>>,
		options: XmlParserOptions<'a, E>,
	) -> impl 'a + Future<Output = Result<XmlParser<'a, E>, E>> {
		async move {
			Self {
				input,
				mode: Mode::Xml1_1,
				phantom: PhantomData,
			}
			.pipe(Ok)
		}
	}
}

pub struct XmlWithNamespacesParser<'a, E> {
	input: &'a mut XmlParser<'a, E>,
}

#[derive(PartialEq)]
enum ItemState {
	Ready,
	Dirty,
	Finished,
}
