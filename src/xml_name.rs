use alloc::{string::String, sync::Arc};
use core::{
	cmp::Ordering,
	hash::{Hash, Hasher},
	ops::Deref,
	pin::Pin,
	ptr,
};
use tracing::instrument;

pub struct XmlName {
	local_name: String,
	namespace: Token<str>,
}

#[derive(Debug, Clone)]
pub struct Token<T: ?Sized>(Pin<Arc<T>>);

impl<T: ?Sized> Token<T> {
	#[instrument(skip(this))]
	pub fn as_ptr(this: &Self) -> *const T {
		&*this.0 as *const T
	}
}

impl<T: ?Sized> PartialEq for Token<T> {
	#[instrument(skip(self, other))]
	fn eq(&self, other: &Self) -> bool {
		ptr::eq(Self::as_ptr(self), Self::as_ptr(other))
	}
}

impl<T: ?Sized> PartialEq<T> for Token<T>
where
	T: PartialEq,
{
	#[instrument(skip(self, other))]
	fn eq(&self, other: &T) -> bool {
		&*self.0 == other
	}
}

impl<T: ?Sized> Eq for Token<T> {}

impl<T: ?Sized> PartialOrd for Token<T> {
	#[instrument(skip(self, other))]
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

impl<T: ?Sized> PartialOrd<T> for Token<T>
where
	T: PartialOrd,
{
	#[instrument(skip(self, other))]
	fn partial_cmp(&self, other: &T) -> Option<Ordering> {
		(**self).partial_cmp(other)
	}
}

impl<T: ?Sized> Ord for Token<T> {
	#[instrument(skip(self, other))]
	fn cmp(&self, other: &Self) -> Ordering {
		Ord::cmp(&Self::as_ptr(self), &Self::as_ptr(other))
	}
}

impl<T: ?Sized> Deref for Token<T> {
	type Target = T;

	#[instrument(skip(self))]
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T: ?Sized> Hash for Token<T> {
	#[instrument(skip(self, state))]
	fn hash<H>(&self, state: &mut H)
	where
		H: Hasher,
	{
		Self::as_ptr(self).hash(state);
	}
}

impl<T> Default for Token<T>
where
	T: Default,
{
	#[instrument]
	fn default() -> Self {
		Self(Arc::pin(T::default()))
	}
}
