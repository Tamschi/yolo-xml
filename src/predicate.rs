use core::{
	future::{ready, Future, Ready},
	pin::Pin,
};
use pin_project::pin_project;

pub trait Predicate<T> {
	fn test<'a>(self: Pin<&'a mut Self>, value: &'a T) -> Pin<&'a mut dyn Future<Output = bool>>;
}

pub trait IntoPredicate<T>: Sized {
	type Target: Predicate<T>;
	fn into_predicate(self) -> Self::Target;
}

#[pin_project]
pub struct BlockingPredicate<P> {
	predicate: P,
	future: Option<Ready<bool>>,
}

impl<T, P> Predicate<T> for BlockingPredicate<P>
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

pub fn from_blocking<P, T>(predicate: P) -> BlockingPredicate<P>
where
	P: FnMut(&T) -> bool,
{
	BlockingPredicate {
		predicate,
		future: None,
	}
}

impl<P, T> IntoPredicate<T> for P
where
	P: FnMut(&T) -> bool,
{
	type Target = BlockingPredicate<P>;
	fn into_predicate(self) -> Self::Target {
		from_blocking(self)
	}
}
