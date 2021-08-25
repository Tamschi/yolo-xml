use crate::{peek_stream::PeekStream, Error, ItemState, PEEK};
use alloc::boxed::Box;
use core::{cell::UnsafeCell, pin::Pin, ptr::NonNull};
use futures_core::{Future, TryStream};
use tap::{Pipe, Tap};

pub struct XmlElement<'a, Input: TryStream<Item = u8>> {
	parent: Option<&'a XmlElement<'a, Input>>,
	guts: UnsafeCell<Guts<Input>>,
}

pub struct XmlElementChildren<'a, Input: TryStream<Item = u8>>(
	&'a XmlElement<'a, Input>,
	&'a mut Guts<Input>,
);

struct Guts<Input: TryStream<Item = u8>> {
	parent: *mut Guts<Input>,
	input: NonNull<PeekStream<Input, PEEK>>,
	state: ItemState,
}

impl<Input: TryStream<Item = u8>> Guts<Input> {
	unsafe fn new(input: NonNull<PeekStream<Input, PEEK>>, parent: *mut Guts<Input>) -> Self {
		Self {
			parent: parent.tap(|parent| {
				if let Some(parent) = parent.as_mut() {
					parent.state = ItemState::Dirty;
				}
			}),
			input,
			state: ItemState::Ready,
		}
	}
}

impl<Input: TryStream<Item = u8>> Drop for Guts<Input> {
	fn drop(&mut self) {
		match self.state {
			ItemState::Dirty => (),
			ItemState::Finished => {
				if let Some(parent) = unsafe { self.parent.as_mut() } {
					parent.state = ItemState::Ready;
				}
			}
			ItemState::Ready => todo!(),
		}
	}
}

impl<'a, Input: TryStream<Item = u8>> XmlElement<'a, Input> {
	pub fn next_child(
		&mut self,
	) -> impl '_ + Future<Output = Result<Option<XmlElement<'_, Input>>, Error>> {
		unsafe { &mut *self.guts.get() }.next_child(self)
	}

	pub fn remaining_children_by_ref(
		&mut self,
	) -> (&'_ XmlElement<'a, Input>, XmlElementChildren<'_, Input>) {
		(
			self,
			XmlElementChildren(self, self.guts.get().pipe(|guts| unsafe { &mut *guts })),
		)
	}

	pub async fn finish(&mut self) -> Result<(), Error> {
		while let Some(mut child) = self.next_child().await? {
			let recursive: Pin<Box<dyn Future<Output = _>>> = Box::pin(child.finish());
			recursive.await?;
		}
		Ok(())
	}
}

impl<'a, Input: TryStream<Item = u8>> XmlElementChildren<'a, Input> {
	pub fn next_child(
		&mut self,
	) -> impl '_ + Future<Output = Result<Option<XmlElement<'_, Input>>, Error>> {
		self.1.next_child(self.0)
	}
}

impl<Input: TryStream<Item = u8>> Guts<Input> {
	async fn next_child(
		&mut self,
		owner: &XmlElement<'_, Input>,
	) -> Result<Option<XmlElement<'_, Input>>, Error> {
		match self.state {
			ItemState::Finished => None,
			ItemState::Dirty => panic!(todo!()),
			ItemState::Ready => Some(todo!()),
		}
		.pipe(Ok)
	}
}
