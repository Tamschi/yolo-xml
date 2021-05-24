use core::{future::Future, pin::Pin};

pub trait Predicate<T> {
	fn test<'a>(self: Pin<&'a mut Self>, value: &'a T) -> Pin<&'a mut dyn Future<Output = bool>>;
}

impl<P, T> Predicate<T> for P
where
	P: Unpin + for<'a> FnMut(&'a T) -> Pin<&'a mut dyn Future<Output = bool>>,
{
	fn test<'a>(
		mut self: Pin<&'a mut Self>,
		value: &'a T,
	) -> Pin<&'a mut dyn Future<Output = bool>> {
		self(value)
	}
}
