use core::{
	cmp::min,
	fmt::{self, Debug, Display, Formatter},
	mem::MaybeUninit,
	slice,
	str::{from_utf8, from_utf8_unchecked, from_utf8_unchecked_mut},
};
use miette::Diagnostic;
use std::{
	cell::RefCell,
	mem,
	ops::Range,
	ptr::{addr_of, addr_of_mut, copy_nonoverlapping},
};
use tap::{Pipe, TryConv};
use this_is_fine::Fine;
use thiserror::Error;

/// Input buffer for the XML parser.
///
/// Must be large enough for token lookahead (so about at least 10 bytes should be easily enough).
pub struct StrBuf<'a> {
	origin: *mut MaybeUninit<u8>,
	memory: &'a mut [MaybeUninit<u8>],
	initialized: usize,
	filled: usize,
	validated: usize,
}

impl Debug for StrBuf<'_> {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		struct Digest<'a>(&'a str, usize, &'a str);
		impl Display for Digest<'_> {
			fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
				struct Print<T>(RefCell<T>);
				impl<T: Iterator<Item = char>> Display for Print<T> {
					fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
						for c in &mut *self.0.borrow_mut() {
							write!(f, "{}", c)?
						}
						Ok(())
					}
				}

				if self.0.chars().take(self.1 + 1).count() <= self.1 {
					write!(f, "{}", self.0)
				} else {
					let count = self.1 - self.2.chars().count();
					let half = count / 2;
					write!(
						f,
						"{}{}{}",
						Print(self.0.chars().take(half + count % 2).into()),
						self.2,
						Print(
							self.0
								.chars()
								.rev()
								.take(half)
								.collect::<Vec<_>>()
								.into_iter()
								.rev()
								.into()
						)
					)
				}
			}
		}

		write!(
			f,
			"{}̝{}+{:?}",
			Digest(self.validated(), 20, "⸌⸍"),
			Digest(
				&String::from_utf8_lossy(self.unvalidated_filled()),
				20,
				"⸌⸍"
			),
			self.remaining_len()
		)
	}
}

impl<'a> StrBuf<'a> {
	#[must_use]
	pub fn new(memory: &'a mut [MaybeUninit<u8>]) -> Self {
		Self {
			origin: memory.as_mut_ptr(),
			memory,
			initialized: 0,
			filled: 0,
			validated: 0,
		}
	}

	#[must_use]
	pub fn validated(&self) -> &str {
		unsafe { from_utf8_unchecked(&*(addr_of!(self.memory[0..self.validated]) as *const [u8])) }
	}

	#[must_use]
	pub fn validated_mut(&mut self) -> &mut str {
		unsafe {
			from_utf8_unchecked_mut(
				&mut *(addr_of_mut!(self.memory[0..self.validated]) as *mut [u8]),
			)
		}
	}

	#[must_use]
	pub fn filled(&self) -> &[u8] {
		unsafe { &*(addr_of!(self.memory[0..self.filled]) as *const [u8]) }
	}

	#[must_use]
	pub fn maybe_uninitialized(&self) -> &[MaybeUninit<u8>] {
		self.memory
	}

	#[must_use]
	pub fn unvalidated_filled(&self) -> &[u8] {
		unsafe { &*(addr_of!(self.memory[self.validated..self.filled]) as *const [u8]) }
	}

	#[must_use]
	pub fn remaining_initialized(&mut self) -> &mut [u8] {
		unsafe { &mut *(addr_of_mut!(self.memory[self.filled..self.initialized]) as *mut [u8]) }
	}

	#[must_use]
	pub fn remaining_maybe_uninitialized(&mut self) -> &mut [MaybeUninit<u8>] {
		self.initialized = self.filled;
		&mut self.memory[self.filled..]
	}

	/// # Safety
	///
	/// The first n "remaining" bytes must have been and still be initialised.
	///
	/// # Panics
	///
	/// Iff trying to assume more bytes as filled than there are remaining.
	pub unsafe fn assume_filled_n_remaining(&mut self, n: usize) {
		self.filled = match self.filled.checked_add(n) {
			Some(filled) if filled <= self.memory.len() => filled,
			Some(overfilled) => panic!(
				"Tried to mark first {} of {} bytes as filled.",
				overfilled,
				self.memory.len()
			),
			None => panic!("`usize` overflow ({} + {}).", self.filled, n),
		};
		self.initialized = self.filled + n;
	}

	pub fn fuzzy_initialize_n(&mut self, n: usize) -> usize {
		for i in self.initialized..min(n, self.memory.len()) {
			self.memory[i].write(0);
		}
		self.initialized
	}

	pub fn fuzzy_initialize_n_remaining(&mut self, n: usize) -> &mut [u8] {
		self.fuzzy_initialize_n(self.filled + n);
		self.remaining_initialized()
	}

	pub fn validate(&mut self) -> Fine<&mut str, Utf8Error> {
		match from_utf8(&self.filled()[self.validated..]) {
			Ok(_) => self.validated = self.filled,
			Err(e) => {
				self.validated += e.valid_up_to();
				if let Some(len) = e.error_len() {
					return (self.validated_mut(), Err(Utf8Error { len }));
				}
			}
		}
		return (self.validated_mut(), Ok(()));
	}

	pub fn invalidate(&mut self) {
		self.validated = 0;
	}

	pub fn shift_validated(&mut self, len: usize) -> Result<&'a mut str, OutOfBoundsError> {
		if self.validated < len {
			Err(OutOfBoundsError::new())
		} else {
			self.validated -= len;
			self.filled -= len;
			self.initialized -= len;
			let (validated, memory) = mem::take(&mut self.memory).split_at_mut(len);
			self.memory = memory;
			Ok(unsafe { &mut *(validated as *mut [MaybeUninit<u8>] as *mut str) })
		}
	}

	pub fn shift_filled(&mut self, len: usize) -> Result<&'a mut [u8], OutOfBoundsError> {
		if self.filled < len {
			Err(OutOfBoundsError::new())
		} else {
			self.validated = self.validated.saturating_sub(len);
			self.filled -= len;
			self.initialized -= len;
			let (filled, memory) = mem::take(&mut self.memory).split_at_mut(len);
			self.memory = memory;
			Ok(unsafe { &mut *(filled as *mut [MaybeUninit<u8>] as *mut [u8]) })
		}
	}

	/// Skips past one occurrence of `data` at the beginning of this buffer if present, returning whether it did so.
	///
	/// # Errors
	///
	/// Iff this buffer does not contain enough data to determine whether the data stream begins with `data`.
	pub fn shift_known_array<const LEN: usize>(
		&mut self,
		data: &[u8; LEN],
	) -> Result<Option<&'a mut [u8; LEN]>, Indeterminate> {
		if self.filled < LEN {
			if *self.filled() == data[..self.filled] {
				Err(Indeterminate::new())
			} else {
				Ok(None)
			}
		} else {
			if unsafe { *(addr_of!(self.memory[0..LEN]).cast::<[u8; LEN]>()) } == *data {
				self.validated = self.validated.saturating_sub(LEN);
				self.filled -= LEN;
				self.initialized -= LEN;
				let (skipped, memory) = self.memory.split_at_mut(LEN);
				self.memory = unsafe { &mut *(memory as *mut _) };
				Some(unsafe {
					&mut *(skipped
						.try_conv::<&mut [MaybeUninit<u8>; LEN]>()
						.expect("unreachable") as *mut [MaybeUninit<u8>; LEN])
						.cast::<[u8; LEN]>()
				})
			} else {
				None
			}
			.pipe(Ok)
		}
	}

	/// Skips past a section at the beginning of the buffer if `predicate` returns true for it.
	///
	/// # Errors
	///
	/// Iff this buffer does not contain enough data to supply `predicate`'s input.
	pub fn shift_array_test_full<const LEN: usize>(
		&mut self,
		predicate: impl FnOnce(&[u8; LEN]) -> bool,
	) -> Result<Option<&'a mut [u8; LEN]>, Indeterminate> {
		if self.filled < LEN {
			Err(Indeterminate::new())
		} else {
			if predicate(unsafe { &*(addr_of!(self.memory[0..LEN]).cast::<[u8; LEN]>()) }) {
				self.validated = self.validated.saturating_sub(LEN);
				self.filled -= LEN;
				self.initialized -= LEN;
				let (skipped, memory) = self.memory.split_at_mut(LEN);
				self.memory = unsafe { &mut *(memory as *mut _) };
				Some(unsafe {
					&mut *(skipped
						.try_conv::<&mut [MaybeUninit<u8>; LEN]>()
						.expect("unreachable") as *mut [MaybeUninit<u8>; LEN])
						.cast::<[u8; LEN]>()
				})
			} else {
				None
			}
			.pipe(Ok)
		}
	}

	/// Skips past contiguous bytes at the beginning of the buffer that fulfill `predicate`.
	///
	/// # Errors
	///
	/// Iff this buffer does not contain at least one byte, then [`Err<Indeterminate>`] is returned instead.
	pub fn shift_bytes_while(
		&mut self,
		mut predicate: impl FnMut(u8) -> bool,
	) -> Result<&'a mut [u8], Indeterminate> {
		if self.filled < 1 {
			Err(Indeterminate::new())
		} else {
			let mut count = 0;
			while count < self.filled
				&& predicate(unsafe { *(addr_of!(self.memory[count]).cast()) })
			{
				count += 1;
			}
			Ok(self.shift_filled(count).expect("unreachable"))
		}
	}

	/// Returns the number of bytes that can still be inserted into this buffer in the current memory allocation (without resetting it).
	#[must_use]
	pub fn remaining_len(&self) -> usize {
		self.memory.len() - self.filled
	}

	#[must_use]
	pub fn into_filled_raw_parts(self) -> Range<*mut u8> {
		self.memory[0].as_mut_ptr()..self.memory[self.filled].as_mut_ptr()
	}

	pub fn clone_into<'b>(
		&self,
		memory: &'b mut [MaybeUninit<u8>],
	) -> Result<StrBuf<'b>, OutOfBoundsError> {
		if self.filled > memory.len() {
			Err(OutOfBoundsError::new())
		} else {
			memory[..self.filled].copy_from_slice(&self.memory[..self.filled]);
			Ok(StrBuf {
				origin: memory.as_mut_ptr(),
				memory,
				initialized: self.filled,
				filled: self.filled,
				validated: self.validated,
			})
		}
	}

	/// Resets the buffer to span its originally covered memory region.
	///
	/// Remaining data is moved to the start of that memory.
	///
	/// # Returns
	///
	/// The distance the buffer was shifted (in bytes).
	///
	/// > This can summed up to keep track of the cursor position in an XML document.
	/// >
	/// > This should, however, be done carefully to not implicitly restrict the length of accepted XML streams.
	///
	/// # Safety
	///
	/// This method invalidates the entire memory region this instance of [`StrBuf`] was initialised with.
	///
	/// All borrows of buffer data (like those extracted through the `.shift_*` methods) must have been released.
	pub unsafe fn unshift_reset(&mut self) -> usize {
		let filled = (self
			.memory
			.as_mut_ptr()
			.offset_from(self.origin)
			.try_conv::<usize>()
			.expect("unreachable"))
			..(addr_of_mut!(self.memory[self.filled])
				.offset_from(self.origin)
				.try_conv::<usize>()
				.expect("unreachable"));

		let original_length = self
			.memory
			.as_mut_ptr_range()
			.end
			.offset_from(self.origin)
			.try_conv::<usize>()
			.expect("unreachable");

		self.memory = &mut [];
		self.memory = slice::from_raw_parts_mut(self.origin, original_length);

		if filled.len() > filled.start {
			// Slow path.
			self.memory.copy_within(filled.clone(), 0);
		} else {
			// Fast path.
			copy_nonoverlapping(
				addr_of_mut!(self.memory[filled.start]),
				addr_of_mut!(self.memory[0]),
				filled.len(),
			);
		}

		self.initialized += filled.start;

		filled.start
	}

	/// Checks whether the buffer has shifted from its original start position.
	#[must_use]
	pub fn is_at_origin(&self) -> bool {
		self.origin as *const _ == self.memory.as_ptr_range().start
	}
}

#[derive(Debug, Error, Diagnostic)]
#[error("Invalid UTF-8 encountered.")]
pub struct Utf8Error {
	len: usize,
}

#[derive(Debug, Error, Diagnostic)]
#[error("Tried to consume or skip data past the amount currently available.")]
pub struct OutOfBoundsError {
	_private: (),
}
impl OutOfBoundsError {
	pub(crate) fn new() -> Self {
		Self { _private: () }
	}
}

#[derive(Debug)]
pub struct Indeterminate {
	_private: (),
}
impl Indeterminate {
	pub(crate) fn new() -> Self {
		Self { _private: () }
	}
}
