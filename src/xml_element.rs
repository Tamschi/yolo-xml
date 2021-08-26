use crate::{peek_stream::PeekStream, xml_name::XmlName, Error, ItemState, PEEK};
use alloc::{boxed::Box, string::String};
use core::{cell::UnsafeCell, pin::Pin, ptr::NonNull};
use futures_core::{Future, TryStream};
use tap::{Pipe, Tap};
use tracing::instrument;

pub struct XmlElement<'a, Input: TryStream<Item = u8>> {
	parent: Option<&'a XmlElement<'a, Input>>,
	guts: UnsafeCell<Guts<Input>>,
	tag_name: XmlName,
	attributes: alloc::collections::BTreeMap<XmlName, String>,
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
	#[instrument(skip(input))]
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

impl<'a, Input: TryStream<Item = u8>> XmlElement<'a, Input> {
	#[instrument(skip(self))]
	pub async fn next_child(&mut self) -> Result<Option<XmlElement<'_, Input>>, Error> {
		unsafe { &mut *self.guts.get() }.next_child(self).await
	}

	#[instrument(skip(self))]
	pub fn remaining_children_by_ref(
		&mut self,
	) -> (&'_ XmlElement<'a, Input>, XmlElementChildren<'_, Input>) {
		(
			self,
			XmlElementChildren(self, self.guts.get().pipe(|guts| unsafe { &mut *guts })),
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

impl<'a, Input: TryStream<Item = u8>> XmlElementChildren<'a, Input> {
	#[instrument(skip(self))]
	pub async fn next_child(&mut self) -> Result<Option<XmlElement<'_, Input>>, Error> {
		self.1.next_child(self.0).await
	}
}

impl<Input: TryStream<Item = u8>> Guts<Input> {
	#[instrument(skip(self, owner))]
	async fn next_child(
		&mut self,
		owner: &XmlElement<'_, Input>,
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
