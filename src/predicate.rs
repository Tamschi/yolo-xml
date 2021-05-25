use core::{
	future::{ready, Future, Ready},
	pin::Pin,
};
use pin_project::pin_project;

pub trait Predicate<T> {
	fn test<'a>(self: Pin<&'a mut Self>, value: &'a T) -> Pin<&'a mut dyn Future<Output = bool>>;
}

#[allow(clippy::module_name_repetitions)]
pub trait IntoPredicate<T>: Sized {
	type IntoPredicate: Predicate<T>;
	fn into_predicate(self) -> Self::IntoPredicate;
}

#[pin_project]
pub struct Blocking<P> {
	predicate: P,
	future: Option<Ready<bool>>,
}
impl<P, T> IntoPredicate<T> for Blocking<P>
where
	P: FnMut(&T) -> bool,
{
	type IntoPredicate = Self;
	fn into_predicate(self) -> Self::IntoPredicate {
		self
	}
}

impl<T, P> Predicate<T> for Blocking<P>
where
	P: FnMut(&T) -> bool,
{
	fn test<'a>(
		mut self: Pin<&'a mut Self>,
		value: &'a T,
	) -> Pin<&'a mut dyn Future<Output = bool>> {
		self.future = Some(ready((self.predicate)(value)));
		Pin::<&mut Ready<bool>>::new(unsafe {
			// Safety: Constrained by return type.
			extend_reference_mut(self.future.as_mut().unwrap())
		})
	}
}

unsafe fn extend_reference_mut<T>(reference: &mut T) -> &'static mut T {
	&mut *(reference as *mut _)
}

pub fn from_blocking<P, T>(predicate: P) -> Blocking<P>
where
	P: FnMut(&T) -> bool,
{
	Blocking {
		predicate,
		future: None,
	}
}

impl<P, T> IntoPredicate<T> for P
where
	P: FnMut(&T) -> bool,
{
	type IntoPredicate = Blocking<P>;
	fn into_predicate(self) -> Self::IntoPredicate {
		from_blocking(self)
	}
}
