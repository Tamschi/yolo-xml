use alloc::{string::String, sync::Arc};
use core::{
	cmp::Ordering,
	hash::{Hash, Hasher},
	ops::Deref,
	pin::Pin,
	ptr,
};

pub struct XmlName {
	local_name: String,
	namespace: Token<str>,
}

#[derive(Debug, Clone)]
pub struct Token<T: ?Sized>(Pin<Arc<T>>);

impl<T: ?Sized> Token<T> {
	pub fn as_ptr(this: &Self) -> *const T {
		&*this.0 as *const T
	}
}

impl<T: ?Sized> PartialEq for Token<T> {
	fn eq(&self, other: &Self) -> bool {
		ptr::eq(Self::as_ptr(self), Self::as_ptr(other))
	}
}

impl<T: ?Sized> PartialEq<T> for Token<T>
where
	T: PartialEq,
{
	fn eq(&self, other: &T) -> bool {
		&*self.0 == other
	}
}

impl<T: ?Sized> Eq for Token<T> {}

impl<T: ?Sized> PartialOrd for Token<T> {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

impl<T: ?Sized> PartialOrd<T> for Token<T>
where
	T: PartialOrd,
{
	fn partial_cmp(&self, other: &T) -> Option<Ordering> {
		(**self).partial_cmp(other)
	}
}

impl<T: ?Sized> Ord for Token<T> {
	fn cmp(&self, other: &Self) -> Ordering {
		Ord::cmp(&Self::as_ptr(self), &Self::as_ptr(other))
	}
}

impl<T: ?Sized> Deref for Token<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T: ?Sized> Hash for Token<T> {
	fn hash<H>(&self, state: &mut H)
	where
		H: Hasher,
	{
		Self::as_ptr(self).hash(state);
	}
}

impl<T: Default> Default for Token<T> {
	fn default() -> Self {
		Self(Arc::pin(Default::default()))
	}
}
