use crate::{peek_stream::PeekStream, ItemState, PEEK};
use core::{cell::UnsafeCell, ptr::NonNull};
use futures_core::TryStream;
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
		if self.state != ItemState::Dirty {
			if let Some(parent) = unsafe { self.parent.as_mut() } {
				parent.state = ItemState::Ready;
			}
		}
	}
}

impl<'a, Input: TryStream<Item = u8>> XmlElement<'a, Input> {
	pub fn next_child(&mut self) -> Option<XmlElement<'_, Input>> {
		unsafe { &mut *self.guts.get() }.next_child(self)
	}

	pub fn remaining_children(
		&mut self,
	) -> (&'_ XmlElement<'a, Input>, XmlElementChildren<'_, Input>) {
		(
			self,
			XmlElementChildren(
				self,
				self.guts.get().pipe(|guts| unsafe {
					// SAFETY: Just narrowing mutability to an `UnsafeCell`.
					&mut *guts
				}),
			),
		)
	}
}

impl<'a, Input: TryStream<Item = u8>> XmlElementChildren<'a, Input> {
	pub fn next_child(&mut self) -> Option<XmlElement<'_, Input>> {
		self.1.next_child(self.0)
	}
}

impl<Input: TryStream<Item = u8>> Guts<Input> {
	fn next_child(&mut self, owner: &XmlElement<Input>) -> Option<XmlElement<'_, Input>> {
		todo!()
	}
}
