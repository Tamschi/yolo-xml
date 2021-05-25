use core::{
	cell::UnsafeCell,
	future::Future,
	intrinsics::transmute,
	ops::{Deref, DerefMut},
	pin::Pin,
	ptr::NonNull,
	task::{Context, Poll},
};
use pin_project::pin_project;
use tap::Pipe;

pub trait Runnable<Args, R> {
	fn run(&self, args: Args) -> R;
}

pub struct RunOnce<'a, F: 'a + ?Sized>(&'a F);
impl<'a, F: ?Sized> RunOnce<'a, F> {
	pub fn new(f: &'a F) -> Self {
		Self(f)
	}
}
impl<'a> RunOnce<'a, dyn Runnable<(), ()>> {
	pub fn run(self) {
		self.0.run(())
	}
}

pub struct PinHandle<'a, T: ?Sized> {
	pin: Pin<&'a mut T>,
	on_drop: Option<RunOnce<'a, dyn 'a + Runnable<(), ()>>>,
}
impl<'a, T: ?Sized> PinHandle<'a, T> {
	#[must_use]
	pub fn new(
		pin: Pin<&'a mut T>,
		on_drop: Option<RunOnce<'a, dyn 'a + Runnable<(), ()>>>,
	) -> Self {
		Self { pin, on_drop }
	}
}
impl<'a, T: ?Sized> Deref for PinHandle<'a, T> {
	type Target = Pin<&'a mut T>;
	fn deref(&self) -> &Self::Target {
		&self.pin
	}
}
impl<'a, T: ?Sized> DerefMut for PinHandle<'a, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.pin
	}
}
impl<'a, T: ?Sized> Drop for PinHandle<'a, T> {
	fn drop(&mut self) {
		self.on_drop.take().map(RunOnce::run).unwrap_or_default()
	}
}
impl<'a, T: ?Sized> Future for PinHandle<'a, T>
where
	T: Future,
{
	type Output = T::Output;

	fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		self.pin.as_mut().poll(cx)
	}
}

pub trait Predicate<T: ?Sized> {
	fn test<'a>(
		self: Pin<&'a mut Self>,
		value: &'a T,
	) -> PinHandle<'a, dyn 'a + Future<Output = bool>>;
}

#[allow(clippy::module_name_repetitions)]
pub trait IntoPredicate<T: ?Sized>: Sized {
	type IntoPredicate: Predicate<T>;
	#[must_use]
	fn into_predicate(self) -> Self::IntoPredicate;
}

#[pin_project]
pub struct Blocking<P, T: ?Sized>
where
	P: FnMut(&T) -> bool,
{
	predicate: UnsafeCell<P>,
	param: Option<NonNull<T>>,
	/// Please audit: Is this enough or do I need an atomic here wrt. weaker memory ordering on ARM-based systems?
	result: UnsafeCell<Option<bool>>,
}
unsafe impl<P, T: ?Sized> Send for Blocking<P, T>
where
	P: Send + FnMut(&T) -> bool,
	T: Sync,
{
}
impl<P, T: ?Sized> IntoPredicate<T> for Blocking<P, T>
where
	P: FnMut(&T) -> bool,
{
	type IntoPredicate = Self;
	fn into_predicate(self) -> Self::IntoPredicate {
		self
	}
}

#[repr(transparent)]
#[pin_project]
// So *in theory* the `UnsafeCell` here should mean a `Pin<&mut BlockingFuture>` doesn't count as mutable reference to the underlying data.
struct BlockingFuture<P, T: ?Sized>(#[pin] UnsafeCell<Blocking<P, T>>)
where
	P: FnMut(&T) -> bool;
unsafe impl<P, T: ?Sized> Send for BlockingFuture<P, T>
where
	P: Send + FnMut(&T) -> bool,
	T: Sync,
{
}
unsafe impl<P, T: ?Sized> Sync for BlockingFuture<P, T>
where
	P: FnMut(&T) -> bool,
{
	// Safety: Shared references to this type aren't interactive.
}
impl<P, T: ?Sized> Future for BlockingFuture<P, T>
where
	P: FnMut(&T) -> bool,
{
	type Output = bool;
	fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
		let blocking = unsafe { &*self.project().0.get() };
		match unsafe { *blocking.result.get() } {
			Some(ready) => ready,
			None => {
				let result =
					unsafe { (*blocking.predicate.get())(blocking.param.unwrap().as_ref()) };
				unsafe {
					*blocking.result.get() = Some(result);
				}
				result
			}
		}
		.pipe(Poll::Ready)
	}
}

#[repr(transparent)]
struct BlockingClear<P, T: ?Sized>(Blocking<P, T>)
where
	P: FnMut(&T) -> bool;
impl<P, T: ?Sized> Runnable<(), ()> for BlockingClear<P, T>
where
	P: FnMut(&T) -> bool,
{
	fn run(&self, _: ()) {
		unsafe {
			*self.0.result.get() = None;
		}
	}
}

impl<P, T: ?Sized> Predicate<T> for Blocking<P, T>
where
	P: FnMut(&T) -> bool,
{
	#[must_use]
	fn test<'a>(
		mut self: Pin<&'a mut Self>,
		value: &'a T,
	) -> PinHandle<'a, dyn 'a + Future<Output = bool>> {
		self.param = Some(value.into());
		let this = &*self;
		PinHandle::new(
			unsafe { transmute::<&Self, Pin<&mut BlockingFuture<P, T>>>(this) },
			Some(RunOnce::new(unsafe {
				&*(this as *const Self).cast::<BlockingClear<P, T>>()
			})),
		)
	}
}

#[must_use]
pub fn from_blocking<P, T: ?Sized>(predicate: P) -> Blocking<P, T>
where
	P: FnMut(&T) -> bool,
{
	Blocking {
		predicate: predicate.into(),
		param: None,
		result: None.into(),
	}
}

impl<P, T: ?Sized> IntoPredicate<T> for P
where
	P: FnMut(&T) -> bool,
{
	type IntoPredicate = Blocking<P, T>;
	fn into_predicate(self) -> Self::IntoPredicate {
		from_blocking(self)
	}
}
