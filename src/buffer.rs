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
	io::ErrorKind,
	mem,
	ops::Range,
	ptr::{addr_of, addr_of_mut, copy_nonoverlapping},
};
use tap::{Pipe, TryConv};
use this_is_fine::Fine;
use thiserror::Error;
use utf8_chars::BufReadCharsExt;

/// Input buffer for the XML parser.
///
/// Must be large enough for token lookahead (so about at least 10 bytes should be easily enough).
pub struct StrBuf<'a> {
	origin: *mut MaybeUninit<u8>,
	memory: &'a mut [MaybeUninit<u8>],
	initialized: usize,
	filled: usize,
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
			"{}+{:?}",
			Digest(&String::from_utf8_lossy(self.filled()), 20, "⸌⸍"),
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

	pub fn shift_filled(&mut self, len: usize) -> Result<&'a mut [u8], OutOfBoundsError> {
		if self.filled < len {
			Err(OutOfBoundsError::new())
		} else {
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

	/// Skips past contiguous text at the beginning of the buffer that fulfill `predicate`.
	///
	/// # Errors
	///
	/// Iff this buffer does not contain at least one byte, then [`Err<Indeterminate>`] is returned instead.
	///
	/// TODO
	pub fn shift_chars_while(
		&mut self,
		mut predicate: impl FnMut(char) -> bool,
	) -> Result<Result<&'a mut str, Utf8Error>, Indeterminate> {
		let mut rest = self.filled();
		let mut chars = BufReadCharsExt::chars_raw(&mut rest);
		let mut len = 0;
		loop {
			match chars.next() {
				Some(Err(error)) => match error.as_io_error().kind() {
					ErrorKind::InvalidData => match len {
						0 => {
							return Ok(Err(Utf8Error {
								len: error.as_bytes().len(),
							}));
						}
						_ => break,
					},
					ErrorKind::UnexpectedEof => match len {
						0 => return Err(Indeterminate::new()),
						_ => break,
					},
					_ => unreachable!(),
				},
				None => match len {
					0 => return Err(Indeterminate::new()),
					_ => break,
				},
				Some(Ok(c)) => match predicate(c) {
					true => len += c.len_utf8(),
					false => break,
				},
			};
		}

		Ok(Ok(unsafe {
			//SAFETY: UTF-8 validation happens above.
			from_utf8_unchecked_mut(self.shift_filled(len).expect("unreachable"))
		}))
	}

	/// Skips past contiguous text at the beginning of the buffer that fulfill `predicate`.
	///
	/// # Errors
	///
	/// Iff this buffer does not contain at least one byte, then [`Err<Indeterminate>`] is returned instead.
	///
	/// TODO
	pub fn shift_chars_start_while(
		&mut self,
		start_predicate: impl FnOnce(char) -> bool,
		mut predicate: impl FnMut(char) -> bool,
	) -> Result<Result<&'a mut str, Utf8Error>, Indeterminate> {
		let mut rest = self.filled();
		let mut chars = BufReadCharsExt::chars_raw(&mut rest);
		let mut len = match chars.next() {
			Some(Err(error)) => match error.as_io_error().kind() {
				ErrorKind::InvalidData => {
					return Ok(Err(Utf8Error {
						len: error.as_bytes().len(),
					}))
				}
				ErrorKind::UnexpectedEof => return Err(Indeterminate::new()),
				_ => unreachable!(),
			},
			None => return Err(Indeterminate::new()),
			Some(Ok(c)) => match start_predicate(c) {
				true => c.len_utf8(),
				false => {
					return Ok(Ok(unsafe {
						//SAFETY: The empty slice is always valid UTF-8.
						from_utf8_unchecked_mut(self.shift_filled(0).expect("unreachable"))
					}));
				}
			},
		};
		loop {
			match chars.next() {
				Some(Err(error)) => match error.as_io_error().kind() {
					ErrorKind::InvalidData => match len {
						0 => {
							return Ok(Err(Utf8Error {
								len: error.as_bytes().len(),
							}));
						}
						_ => break,
					},
					ErrorKind::UnexpectedEof => match len {
						0 => return Err(Indeterminate::new()),
						_ => break,
					},
					_ => unreachable!(),
				},
				None => break,
				Some(Ok(c)) => match predicate(c) {
					true => len += c.len_utf8(),
					false => break,
				},
			};
		}

		//TODO: Transform line endings!
		Ok(Ok(unsafe {
			//SAFETY: UTF-8 validation happens above.
			from_utf8_unchecked_mut(self.shift_filled(len).expect("unreachable"))
		}))
	}

	/// TODO
	///
	/// # Errors
	///
	/// Iff this buffer does not contain at least one byte, then [`Err<Indeterminate>`] is returned instead.
	///
	/// TODO
	pub fn shift_chars_delimited(
		&mut self,
		delimiter: &[u8],
	) -> Result<Result<&'a mut str, Utf8Error>, Indeterminate> {
		let data = match self
			.filled()
			.windows(delimiter.len())
			.enumerate()
			.find_map(|(i, window)| (window == delimiter).then_some(i))
		{
			Some(0) => {
				return Ok(Ok(unsafe {
					//SAFETY: The empty slice is always valid UTF-8.
					from_utf8_unchecked_mut(self.shift_filled(0).expect("unreachable"))
				}));
			}
			Some(i) => &self.filled()[..i],
			None => 'partial: {
				if delimiter.starts_with(self.filled()) {
					return Err(Indeterminate::new());
				}
				for n in (0..delimiter.len()).rev() {
					if self.filled().ends_with(&delimiter[..n]) {
						break 'partial &self.filled()[..(self.filled - n)];
					}
				}
				unreachable!()
			}
		};

		debug_assert_ne!(data.len(), 0);

		let valid_len = match from_utf8(data) {
			Ok(valid) => valid.len(),
			Err(error) => match error.valid_up_to() {
				0 => match error.error_len() {
					None => return Err(Indeterminate::new()),
					Some(error_len) => return Ok(Err(Utf8Error { len: error_len })),
				},
				len => len,
			},
		};

		//TODO: Transform line endings!
		Ok(Ok(unsafe {
			//SAFETY: Validate above.
			from_utf8_unchecked_mut(self.shift_filled(valid_len).expect("unreachable"))
		}))
	}

	/// TODO
	///
	/// # Errors
	///
	/// Iff this buffer does not contain at least one byte, then [`Err<Indeterminate>`] is returned instead.
	///
	/// TODO
	pub fn shift_chars_while_delimited(
		&mut self,
		mut predicate: impl FnMut(char) -> bool,
		delimiter: &[u8],
	) -> Result<Result<&'a mut str, Utf8Error>, Indeterminate> {
		let data = match self
			.filled()
			.windows(delimiter.len())
			.enumerate()
			.find_map(|(i, window)| (window == delimiter).then_some(i))
		{
			Some(0) => {
				return Ok(Ok(unsafe {
					//SAFETY: The empty slice is always valid UTF-8.
					from_utf8_unchecked_mut(self.shift_filled(0).expect("unreachable"))
				}));
			}
			Some(i) => &self.filled()[..i],
			None => 'partial: {
				if delimiter.starts_with(self.filled()) {
					return Err(Indeterminate::new());
				}
				for n in (0..delimiter.len()).rev() {
					if self.filled().ends_with(&delimiter[..n]) {
						break 'partial &self.filled()[..(self.filled - n)];
					}
				}
				unreachable!()
			}
		};

		debug_assert_ne!(data.len(), 0);

		let valid_len = match from_utf8(data) {
			Ok(valid) => valid
				.chars()
				.take_while(|c| predicate(*c))
				.map(char::len_utf8)
				.sum(),
			Err(error) => match error.valid_up_to() {
				0 => match error.error_len() {
					None => return Err(Indeterminate::new()),
					Some(error_len) => return Ok(Err(Utf8Error { len: error_len })),
				},
				len => from_utf8(&data[..len])
					.expect("unreachable")
					.chars()
					.take_while(|c| predicate(*c))
					.map(char::len_utf8)
					.sum(),
			},
		};

		//TODO: Transform line endings!
		Ok(Ok(unsafe {
			//SAFETY: Validate above.
			from_utf8_unchecked_mut(self.shift_filled(valid_len).expect("unreachable"))
		}))
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

#[derive(Debug, Error, Diagnostic, PartialEq, Eq)]
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
