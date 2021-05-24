use crate::{fake_discard_callback, Mode};
use core::{
	pin::Pin,
	task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};
use ergo_pin::ergo_pin;
use futures_core::{Future, Stream};
use pin_project::pin_project;
use tap::Pipe;

#[ergo_pin]
fn eval<O>(future: impl Future<Output = O>) -> O {
	const INSOMNIA: RawWakerVTable = RawWakerVTable::new(
		|_| unreachable!(),
		|_| unreachable!(),
		|_| unreachable!(),
		|_| unreachable!(),
	);

	unsafe {
		let waker = Waker::from_raw(RawWaker::new(&() as *const _, &INSOMNIA));
		match pin!(future).poll(&mut Context::from_waker(&waker)) {
			Poll::Ready(output) => output,
			Poll::Pending => unreachable!(),
		}
	}
}

pub struct XmlParserOptions<'a, E: 'a> {
	pub on_mode: &'a mut dyn FnMut(Mode) -> Result<(), E>,
}
impl<'a, E: 'a> Default for XmlParserOptions<'a, E> {
	fn default() -> Self {
		Self {
			on_mode: fake_discard_callback(),
		}
	}
}

/// [`Stream`] is sadly not blanket-implemented for [`Iterator`]s,
/// so a wrapper is necessary to avoid widespread monomorphisation.

// This marks the inner iterator not pinned (somehow),
// allowing use through iterator methods.
#[pin_project]
pub struct Input<I: Iterator<Item = Result<char, E>>, E>(I);
impl<I: Iterator<Item = Result<char, E>>, E> Stream for Input<I, E> {
	type Item = I::Item;

	fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
		self.0.next().pipe(Poll::Ready)
	}
}

pub struct XmlParser<'a, E>(crate::XmlParser<'a, E>);
impl<'a, E> XmlParser<'a, E> {
	pub fn run<I: Iterator<Item = Result<char, E>>, T>(
		input: &'a mut Input<I, E>,
		options: &'a mut XmlParserOptions<'a, E>,
		parse: impl FnOnce(XmlParser<'_, E>) -> Result<T, E>,
	) -> Result<T, E> {
		let XmlParserOptions { on_mode } = options;
		crate::XmlParser::start(input, super::XmlParserOptions { on_mode })
			.pipe(eval)
			.map(Self)
			.and_then(parse)
	}
}
