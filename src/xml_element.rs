use crate::{
	iter::AsyncIterator, peek_stream::PeekStream, xml_name::XmlName, Error, ItemState, PEEK,
};
use alloc::{boxed::Box, string::String};
use core::{cell::UnsafeCell, pin::Pin, ptr::NonNull};
use futures_core::Future;
use tap::{Pipe, Tap};
use tracing::instrument;

pub struct XmlElement<'a, Input: for<'b> AsyncIterator<Item<'b> = Result<u8, E>>, E> {
	parent: Option<&'a XmlElement<'a, Input, E>>,
	guts: UnsafeCell<Guts<'a, Input, E>>,
	tag_name: XmlName,
	attributes: alloc::collections::BTreeMap<XmlName, String>,
}

pub struct XmlElementChildren<'a, Input: for<'b> AsyncIterator<Item<'b> = Result<u8, E>>, E> {
	parent: &'a XmlElement<'a, Input, E>,
	guts: &'a mut Guts<'a, Input, E>,
}

struct Guts<'a, Input: for<'b> AsyncIterator<Item<'b> = Result<u8, E>>, E> {
	parent: *mut Guts<'a, Input, E>,
	input: NonNull<PeekStream<'a, Input, PEEK>>,
	state: ItemState,
}

impl<'a, Input: for<'b> AsyncIterator<Item<'b> = Result<u8, E>>, E> Guts<'a, Input, E> {
	#[instrument(skip(input))]
	unsafe fn new(
		input: NonNull<PeekStream<Input, PEEK>>,
		parent: *mut Guts<'a, Input, E>,
	) -> Self {
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

impl<'a, Input: for<'b> AsyncIterator<Item<'b> = Result<u8, E>>, E> XmlElement<'a, Input, E> {
	#[instrument(skip(self))]
	pub async fn next_child(&mut self) -> Result<Option<XmlElement<'_, Input>>, Error> {
		unsafe { &mut *self.guts.get() }.next_child(self).await
	}

	#[instrument(skip(self))]
	pub fn remaining_children_by_ref(
		&mut self,
	) -> (
		&'_ XmlElement<'a, Input, E>,
		XmlElementChildren<'_, Input, E>,
	) {
		(
			self,
			XmlElementChildren {
				parent: self,
				guts: self.guts.get().pipe(|guts| unsafe { &mut *guts }),
			},
		)
	}

	#[instrument(skip(self))]
	pub async fn finish(&mut self) -> Result<(), Error> {
		while let Some(mut child) = self.next_child().await? {
			let recursive: Pin<Box<dyn Future<Output = _>>> = Box::pin(child.finish());
			recursive.await?;
		}
		todo!("parse end tag");
		let guts = self.guts.get_mut();
		if let Some(parent) = unsafe { guts.parent.as_mut() } {
			parent.state = ItemState::Ready;
		}
		Ok(())
	}
}

impl<'a, Input: 'a + for<'b> AsyncIterator<Item<'b> = Result<u8, E>>, E>
	XmlElementChildren<'a, Input, E>
{
	#[instrument(skip(self))]
	pub async fn next_child(&mut self) -> Result<Option<XmlElement<'_, Input>>, Error> {
		self.guts.next_child(self.parent).await
	}
}

impl<'a, Input: for<'b> AsyncIterator<Item<'b> = Result<u8, E>>, E> Guts<'a, Input, E> {
	#[instrument(skip(self, owner))]
	async fn next_child(
		&mut self,
		owner: &XmlElement<'_, Input, E>,
	) -> Result<Option<XmlElement<'_, Input>>, Error> {
		match self.state {
			ItemState::Finished => None,
			ItemState::Dirty => {
				panic!("Element is dirty. `.finish().await?` each child before continuing.")
			}
			ItemState::Ready => Some(todo!()),
		}
		.pipe(Ok)
	}
}
