use core::{
	pin::Pin,
	task::{Context, Poll},
};

pub trait AsyncIterator {
	type Item<'a>
	where
		Self: 'a;

	fn poll_next<'a>(self: Pin<&'a mut Self>, cx: &mut Context<'_>)
		-> Poll<Option<Self::Item<'a>>>;

	fn size_hint(&self) -> (usize, Option<usize>) {
		(0, None)
	}
}
