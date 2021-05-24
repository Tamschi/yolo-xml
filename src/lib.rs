#![doc(html_root_url = "https://docs.rs/yolo-xml/0.0.1")]
#![no_std]
#![warn(clippy::pedantic)]
#![allow(clippy::if_not_else)]

use core::{future::Future, marker::PhantomData, mem::size_of_val, pin::Pin};
use futures_core::Stream;
use peek_stream::PeekStream;
use tap::Pipe as _;

pub mod blocking;
mod peek_stream;

#[cfg(doctest)]
pub mod readme {
	doc_comment::doctest!("../README.md");
}

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

async fn skip_whitespace<Input: Stream<Item = Result<char, E>>, E, const CAPACITY: usize>(
	mut input: Pin<&mut PeekStream<Input, CAPACITY>>,
) -> Result<(), E> {
	while input
		.as_mut()
		.next_if(|next| match next {
			Ok(char) => "\u{20}\u{9}\u{D}\u{A}".contains(*char), // [3] S
			Err(_) => true,
		})
		.await
		.transpose()?
		.is_some()
	{}
	Ok(())
}

pub struct XmlParserOptions<'a, E: 'a> {
	on_mode: &'a mut dyn FnMut(Mode) -> Result<(), E>,
}
impl<'a, E: 'a> Default for XmlParserOptions<'a, E> {
	fn default() -> Self {
		Self {
			on_mode: fake_discard_callback(),
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
