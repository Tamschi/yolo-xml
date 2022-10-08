use core::{
	cmp::min,
	convert::TryFrom,
	fmt::Debug,
	mem::MaybeUninit,
	str::{from_utf8, from_utf8_unchecked, from_utf8_unchecked_mut},
};
use miette::Diagnostic;
use std::ptr::{addr_of, addr_of_mut};
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
		&mut self.memory[self.filled..]
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
				self.validated = e.valid_up_to();
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

	pub fn reset(&mut self) {
		self.validated = 0;
		self.filled = 0;
	}

	pub fn consume_bytes<const N: usize>(&mut self) -> Result<[u8; N], OutOfBoundsError> {
		if self.filled < N {
			return Err(OutOfBoundsError::new());
		}

		let array = <[u8; N]>::try_from(&self.filled()[0..N]).expect("unreachable");
		self.shift(N).expect("unreachable");
		Ok(array)
	}

	pub fn shift(&mut self, n: usize) -> Result<(), OutOfBoundsError> {
		if self.filled < n {
			Err(OutOfBoundsError::new())
		} else {
			for i in 0..min(n, self.initialized - n) {
				self.memory[i] = self.memory[n + i];
			}
			self.validated = self.validated.saturating_sub(n);
			self.filled -= n;
			Ok(())
		}
	}

	#[must_use]
	pub fn remaining_len(&self) -> usize {
		self.memory.len() - self.filled
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
	fn new() -> Self {
		Self { private: () }
	}
}
