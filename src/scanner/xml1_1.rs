#![allow(clippy::enum_glob_use, non_snake_case, clippy::match_same_arms)]

use super::{
	xml1_0, Error, Event, Event_, MoreInputRequired,
	Next::*,
	NextFnR,
	RetVal::{self, *},
	StringType, TokenizedType,
};
use crate::{
	buffer::StrBuf,
	scanner::xml1_0::{Grammar, Xml1_0},
};
use std::any::type_name;
use tap::Pipe;
use tracing::instrument;

fn type_name_of_val<T>(_: T) -> &'static str {
	type_name::<T>()
}

macro_rules! Call {
	($state:expr, $callee:ident) => {
		Call(
			$state,
			Self::$callee,
			#[cfg(debug_assertions)]
			type_name_of_val(Self::$callee),
		)
	};
}

pub(super) enum Xml1_1 {}

impl Grammar for Xml1_1 {
	// [1] `document` unmodified.

	//TODO: [2] `Char` modified!
	//TODO: [2a] `RestrictedChar` added!

	// [3] `S` unmodified.

	// [4] `NameStartChar` unmodified.
	// through
	// [21] `CDEnd` unmodified.

	/// [22]
	/// Start tokens: `<?xml` (but downgrades if missing)
	#[instrument(ret(Debug))]
	fn prolog<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
		match (state, ret_val) {
			(0, _) => Call!(1, XMLDecl),
			(1 | 2, Accept) => Call!(2, Misc),
			(2, Reject) => Call!(3, doctypedecl),
			(3 | 4, Accept) => Call!(4, Misc),
			(3 | 4, Reject) => Exit(Accept),
			(1, Reject) => Yield(0, Event_::RebootToVersion1_0),
			_ => unreachable!(),
		}
		.pipe(Ok)
	}

	// [23] `XMLDecl` unmodified.

	/// [24]
	/// Unmodified, but contains downgrade logic.
	#[instrument(ret(Debug))]
	fn VersionInfo<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
		match (state, ret_val) {
			(0, _) => Call!(1, S),
			(1, Accept) => match buffer.shift_known_array(b"version")? {
				Some(_) => Continue(2),
				None => Error(Error::ExpectedLiteral(b"version")),
			},
			(1, Reject) => Exit(Reject),
			(2, _) => Call!(3, Eq),
			(3, Accept) => match buffer.shift_known_array(b"'")? {
				Some(_) => Continue(4),
				None => match buffer.shift_known_array(b"\"")? {
					Some(_) => Continue(6),
					None => Error(Error::ExpectedQuote),
				},
			},
			(3, Reject) => unreachable!("`Eq` shouldn't fail."),
			(4, _) => Call!(5, VersionNum),
			(5, Accept) => match buffer.shift_known_array(b"'")? {
				Some(_) => Exit(Accept),
				None => Error(Error::ExpectedLiteral(b"'")),
			},
			(5, Reject) => Yield(8, Event_::DowngradeFrom1_1SingleQuoted),
			(6, _) => Call!(7, VersionNum),
			(7, Accept) => match buffer.shift_known_array(b"\"")? {
				Some(_) => Exit(Accept),
				None => Error(Error::ExpectedLiteral(b"\"")),
			},
			(7, Reject) => Yield(8, Event_::DowngradeFrom1_1DoubleQuoted),
			(8, _) => unreachable!("should have downgraded"),
			_ => unreachable!(),
		}
		.pipe(Ok)
	}

	// [25] `Eq` unmodified.

	/// [26]
	/// Must be `1.1` now (and check for termination).
	#[instrument(ret(Debug))]
	fn VersionNum<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
		match (state, ret_val) {
			//BUG: Ensure this is terminated!
			(0, _) => match buffer.shift_known_array(b"1.1")? {
				Some(version) => Yield(1, Event::VersionChunk(version).into()),
				None => Exit(Reject),
			},
			(1, _) => Exit(Accept),
			_ => unreachable!(),
		}
		.pipe(Ok)
	}

	// [27] `Misc` unmodified.

	// [28] `doctypedecl` unmodified.
	// through
	// [32] `SDDecl` unmodified.

	// [39] `element` unmodified.
	// through
	// [77] `TextDecl` unmodified.

	//TODO: [78] `extParsedEnt` modified!

	// [80] `EncodingDecl` unmodified.
	// through
	// [83] `PublicID` unmodified.
}
