use core::{
	convert::TryFrom,
	mem::MaybeUninit,
	num::NonZeroUsize,
	ops::{Add, AddAssign, Sub},
	pin::Pin,
	task::{Context, Poll},
};
use ergo_pin::ergo_pin;
use futures_core::Stream;
use futures_util::StreamExt as _;
use pin_project::pin_project;
use tap::{Conv as _, Pipe as _};

use crate::predicate::{IntoPredicate, Predicate};

// A neat generic implementation isn't yet possible because types of const generic parameters can't depend on other type parameters yet.
// TODO: Check maths terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Modular<const MODULE: usize>(pub usize);
impl<const MODULE: usize> From<Modular<MODULE>> for usize {
	fn from(modular: Modular<MODULE>) -> Self {
		modular.0
	}
}
impl<const MODULE: usize> From<&mut Modular<MODULE>> for usize {
	fn from(modular: &mut Modular<MODULE>) -> Self {
		modular.0
	}
}
impl<const MODULE: usize> TryFrom<usize> for Modular<MODULE> {
	type Error = ();

	fn try_from(linear: usize) -> Result<Self, ()> {
		(linear < MODULE).then(|| Modular(linear)).ok_or(())
	}
}
impl<const MODULE: usize> Sub for Modular<MODULE> {
	type Output = usize;

	fn sub(self, rhs: Self) -> Self::Output {
		if self.0 >= rhs.0 {
			self.0 - rhs.0
		} else {
			MODULE - rhs.0 + self.0
		}
	}
}
impl<Rhs, const MODULE: usize> Sub<&Rhs> for Modular<MODULE>
where
	Self: Sub<Rhs>,
	Rhs: Copy,
{
	type Output = <Self as Sub<Rhs>>::Output;

	fn sub(self, rhs: &Rhs) -> Self::Output {
		self - *rhs
	}
}
impl<Rhs, const MODULE: usize> Sub<Rhs> for &Modular<MODULE>
where
	Modular<MODULE>: Sub<Rhs>,
{
	type Output = <Modular<MODULE> as Sub<Rhs>>::Output;

	fn sub(self, rhs: Rhs) -> Self::Output {
		*self - rhs
	}
}
impl<const MODULE: usize> Add<usize> for Modular<MODULE> {
	type Output = Self;

	fn add(self, rhs: usize) -> Self::Output {
		Modular(
			self.0
				.checked_add(rhs % MODULE)
				.expect("`Module` overflow in `add_assign`")
				% MODULE,
		)
	}
}
impl<const MODULE: usize> Add<usize> for &Modular<MODULE> {
	type Output = Modular<MODULE>;

	fn add(self, rhs: usize) -> Self::Output {
		*self + rhs
	}
}
impl<const MODULE: usize> AddAssign<usize> for Modular<MODULE> {
	fn add_assign(&mut self, rhs: usize) {
		*self = *self + rhs;
	}
}

#[pin_project]
pub struct PeekStream<Input: Stream, const CAPACITY: usize> {
	#[pin]
	input: Input,
	buffer: [MaybeUninit<Input::Item>; CAPACITY],
	start: Modular<CAPACITY>,
	len: usize,
}
impl<Input: Stream, const CAPACITY: usize> Stream for PeekStream<Input, CAPACITY> {
	type Item = Input::Item;

	fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
		let this = self.project();
		if *this.len > 0 {
			let i: usize = this.start.into();
			*this.start += 1;
			*this.len -= 1;
			unsafe { this.buffer[i].as_ptr().read() }
				.pipe(Some)
				.pipe(Poll::Ready)
		} else {
			this.input.poll_next(cx)
		}
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		let (start, end) = self.input.size_hint();
		(
			start + self.len,
			end.and_then(|end| end.checked_add(self.len)),
		)
	}
}
impl<Input: Stream, const CAPACITY: usize> PeekStream<Input, CAPACITY> {
	pub async fn peek_1(self: Pin<&mut Self>) -> Option<&Input::Item> {
		self.peek_n(NonZeroUsize::new(1).unwrap()).await
	}

	pub async fn peek_n(self: Pin<&mut Self>, depth: NonZeroUsize) -> Option<&Input::Item> {
		assert!(
			depth.get() <= CAPACITY,
			"`depth` out of range `0..CAPACITY`"
		);
		let mut this = self.project();
		while *this.len < depth.get() {
			this.buffer[(*this.start + *this.len).conv::<usize>()] =
				this.input.next().await?.pipe(MaybeUninit::new);
			*this.len += 1;
		}
		Some(unsafe {
			// Safety: Assuredly written to directly above or earlier than that.
			&*this.buffer[(*this.start + depth.get()).conv::<usize>()].as_ptr()
		})
	}

	#[ergo_pin]
	pub async fn next_if(
		mut self: Pin<&mut Self>,
		predicate: impl IntoPredicate<Input::Item>,
	) -> Option<Input::Item> {
		let item = self.as_mut().peek_1().await?;
		if pin!(predicate.into_predicate()).test(item).await {
			self.next().await
		} else {
			None
		}
	}
}
