pub trait PinLensMut<F: RefRefractionMut> {
	fn lense(self: Pin<&mut Self>) -> &mut F::Target;
}

mod sealed {
	use core::pin::Pin;

	pub trait Sealed {}
	impl<A: ?Sized> Sealed for fn(&A) -> Pin<&mut (dyn '_ + core::future::Future<Output = bool>)> {}
}
use core::{future::Future, pin::Pin};

use sealed::Sealed;

pub trait RefRefractionMut: Sealed {
	type Target: ?Sized;
}
impl<A: ?Sized> RefRefractionMut for fn(&A) -> Pin<&mut (dyn '_ + Future<Output = bool>)> {
	type Target = dyn FnMut(&A) -> Pin<&mut (dyn '_ + Future<Output = bool>)>;
}
