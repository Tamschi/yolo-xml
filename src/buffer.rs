use core::{
	cmp::min,
	fmt::Debug,
	mem::MaybeUninit,
	str::{from_utf8, from_utf8_unchecked, from_utf8_unchecked_mut},
};
use miette::Diagnostic;
use std::{
	mem,
	ops::Range,
	ptr::{addr_of, addr_of_mut, copy_nonoverlapping},
};
use tap::{Pipe, TryConv};
use this_is_fine::Fine;
use thiserror::Error;

pub struct StrBuf<'a> {
	memory: &'a mut [MaybeUninit<u8>],
	initialized: usize,
	filled: usize,
	validated: usize,
}

impl Debug for StrBuf<'_> {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("StrBuf")
			.field("capacity", &self.memory.len())
			.field("initialized", &self.initialized)
			.field("filled", &self.filled)
			.field("validated", &self.validated)
			.field("validated()", &self.validated())
			.field("unvalidated_filled()", &self.unvalidated_filled())
			.finish()
	}
}

impl<'a> StrBuf<'a> {
	#[must_use]
	pub fn new(memory: &'a mut [MaybeUninit<u8>]) -> Self {
		Self {
			memory,
			initialized: 0,
			filled: 0,
			validated: 0,
		}
	}
}

impl StrBuf<'_> {
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

	pub fn shift_validated(&mut self, len: usize) -> Result<&mut str, OutOfBoundsError> {
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

	pub fn shift_filled(&mut self, len: usize) -> Result<&mut [u8], OutOfBoundsError> {
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
	) -> Result<Option<&mut [u8; LEN]>, OutOfBoundsError> {
		if self.filled < LEN {
			if *self.filled() == data[..self.filled] {
				Err(OutOfBoundsError::new())
			} else {
				Ok(None)
			}
		} else {
			if unsafe { *(addr_of!(self.memory[0]).cast::<[u8; LEN]>()) } == *data {
				self.validated = self.validated.saturating_sub(LEN);
				self.filled -= LEN;
				self.initialized -= LEN;
				let (skipped, memory) = self.memory.split_array_mut();
				Some(unsafe { &mut *(skipped as *mut [MaybeUninit<u8>; LEN]).cast::<[u8; LEN]>() })
			} else {
				None
			}
			.pipe(Ok)
		}
	}

	#[must_use]
	pub fn remaining_len(&self) -> usize {
		self.memory.len() - self.filled
	}

	#[must_use]
	pub fn into_filled_raw_parts(self) -> Range<*mut u8> {
		self.memory[0].as_mut_ptr()..self.memory[self.filled].as_mut_ptr()
	}

	pub fn clone_into<'a>(
		&self,
		memory: &'a mut [MaybeUninit<u8>],
	) -> Result<StrBuf<'a>, OutOfBoundsError> {
		if self.filled > memory.len() {
			Err(OutOfBoundsError::new())
		} else {
			memory[..self.filled].copy_from_slice(&self.memory[..self.filled]);
			Ok(StrBuf {
				memory,
				initialized: self.filled,
				filled: self.filled,
				validated: self.validated,
			})
		}
	}
}

impl<'a> StrBuf<'a> {
	/// (Re-)initialises an [`StrBuf`] over `memory` while retaining the data in `filled`.
	///
	/// # Safety
	///
	/// `filled` must be within `memory` and initialised.
	///
	/// # Panics
	///
	/// This function **may** panic in some cases where its safety constraints are not upheld.
	pub unsafe fn reset(memory: &'a mut [MaybeUninit<u8>], filled: Range<*mut u8>) -> Self {
		if cfg!(debug) {
			'ok: loop {
				'fail: for i in 0..memory.len() {
					if memory[i].as_mut_ptr() == filled.start {
						for i in &mut memory[i..] {
							if i.as_mut_ptr() == filled.end {
								break 'ok;
							}
						}
						break 'fail;
					}
				}
				panic!(
					"`filled` pointer validity check fail. Expected `filled` in `memory` ({:p} <= {:p} <= {:p} <= {:p}).",
					memory.as_mut_ptr_range().start, filled.start, filled.end, memory.as_mut_ptr_range().end,
				);
			}
		}

		let memory_start = addr_of_mut!(memory[0]).cast::<u8>();

		// Note that we mustn't dereference through `filled`, as that would break non-aliasing guarantees!
		let filled = (filled
			.start
			.offset_from(memory_start)
			.try_conv::<usize>()
			.unwrap())
			..(filled
				.end
				.offset_from(memory_start)
				.try_conv::<usize>()
				.unwrap());
		if filled.len() > filled.start {
			// Slow path.
			memory.copy_within(filled.clone(), 0);
		} else {
			// Fast path.
			copy_nonoverlapping(
				addr_of_mut!(memory[filled.start]),
				addr_of_mut!(memory[0]),
				filled.len(),
			);
		}
		Self {
			memory,
			//TODO: This should be more.
			initialized: filled.len(),
			filled: filled.len(),
			validated: 0,
		}
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
	private: (),
}
impl OutOfBoundsError {
	pub(crate) fn new() -> Self {
		Self { private: () }
	}
}
