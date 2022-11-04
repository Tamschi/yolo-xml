#![allow(clippy::enum_glob_use, non_snake_case, clippy::match_same_arms)]

use super::Next::*;
use super::NextFnR;
use super::RetVal;
use super::RetVal::*;
use super::StringType;
use super::TokenizedType;
use super::{Error, Event, Event_, MoreInputRequired};
use crate::buffer::StrBuf;
use std::any::type_name;
use tap::Pipe;
use tracing::instrument;

fn type_name_of_val<T>(_: T) -> &'static str {
	type_name::<T>()
}

macro_rules! Call {
	($state:expr, $callee:expr) => {
		Call(
			$state,
			$callee,
			#[cfg(debug_assertions)]
			type_name_of_val($callee),
		)
	};
}

macro_rules! CallState {
	($state:expr, $callee:expr, $calleState:expr) => {
		CallState(
			$state,
			$callee,
			#[cfg(debug_assertions)]
			type_name_of_val($callee),
			$calleState,
		)
	};
}

pub(super) const START_AT_VERSION_NUMBER_SINGLE_QUOTE: u8 = u8::MAX;
pub(super) const START_AT_VERSION_NUMBER_DOUBLE_QUOTE: u8 = u8::MAX - 1;

/// [1]
#[instrument(ret(Debug))]
pub(super) fn document<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => Call!(1, prolog),
		(1, Accept) => Call!(2, element),
		(2 | 3, Accept) => Call!(3, Misc),
		(3, Reject) => Exit(Accept),
		(1, Reject) => Error(Error::Expected22Prolog),
		(2, Reject) => Error(Error::Expected39Element),

		(
			start_at
			@ (START_AT_VERSION_NUMBER_SINGLE_QUOTE | START_AT_VERSION_NUMBER_DOUBLE_QUOTE),
			_,
		) => CallState!(1, prolog, start_at),

		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [3]
/// Start tokens: *0x20* | *0x9* | *0xD* | *0xA*
#[instrument(ret(Debug))]
pub(super) fn S<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => match buffer.shift_known_array(&[0x20])?.is_some()
			|| buffer.shift_known_array(&[0x9])?.is_some()
			|| buffer.shift_known_array(&[0xD])?.is_some()
			|| buffer.shift_known_array(&[0xA])?.is_some()
		{
			true => Continue(1),
			false => Exit(Reject),
		},
		(1, _) => {
			while buffer.shift_known_array(&[0x20])?.is_some()
				|| buffer.shift_known_array(&[0x9])?.is_some()
				|| buffer.shift_known_array(&[0xD])?.is_some()
				|| buffer.shift_known_array(&[0xA])?.is_some()
			{}
			Exit(Accept)
		}
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [5]
#[instrument(ret(Debug))]
fn Name<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	todo!()
}

/// [10]
#[instrument(ret(Debug))]
fn AttValue<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	todo!()
}

/// [11]
#[instrument(ret(Debug))]
fn SystemLiteral<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	todo!()
}

/// [12]
#[instrument(ret(Debug))]
fn PubidLiteral<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	todo!()
}

/// [16]
#[instrument(ret(Debug))]
fn PI<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => match buffer.shift_known_array(b"<?")? {
			Some(start) => Yield(1, Event::PIStart(start).into()),
			None => Exit(Reject),
		},
		(1, _) => Call!(2, PITarget),
		(2, Accept) => Call!(3, S),
		(2, Reject) => Error(Error::Expected17PITarget),
		(3, Accept) => Continue(4),
		(3, Reject) => match buffer.shift_known_array(b"?>")? {
			Some(end) => Yield(5, Event::PIEnd(end).into()),
			None => Error(Error::ExpectedWhitespaceOrPIEnd),
		},
		(4, _) => {
			if let Some(end) = buffer.shift_known_array(b"?>")? {
				Yield(5, Event::PIEnd(end).into())
			} else {
				match buffer.validate() {
					(valid, Err(error)) if valid.is_empty() => Error(Error::Utf8Error(error)),
					(valid, Ok(())) if valid.is_empty() => return Err(MoreInputRequired::new()),
					//BUG: Detect disallowed characters!
					(_valid, _) => Yield(
						4,
						Event::PIChunk(
							buffer
								.shift_validated(if buffer.validated().ends_with('?') {
									buffer.validated().len() - 1
								} else {
									buffer.validated().len()
								})
								.expect("unreachable"),
						)
						.into(),
					),
				}
			}
		}
		(5, _) => Exit(Accept),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [17]
#[instrument(ret(Debug))]
fn PITarget<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	todo!()
}

/// [22]
#[instrument(ret(Debug))]
pub(super) fn prolog<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => Call!(1, XMLDecl),
		(1, Accept | Reject) | (2, Accept) => Call!(2, Misc),
		(2, Reject) => Call!(3, doctypedecl),
		(3 | 4, Accept) => Call!(4, Misc),
		(3 | 4, Reject) => Exit(Accept),

		(
			start_at
			@ (START_AT_VERSION_NUMBER_SINGLE_QUOTE | START_AT_VERSION_NUMBER_DOUBLE_QUOTE),
			_,
		) => CallState!(1, XMLDecl, start_at),

		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [15]
/// Start tokens: `<!--`
#[instrument(ret(Debug))]
fn Comment<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => match buffer.shift_known_array(b"<!--")? {
			Some(comment_start) => Yield(1, Event::CommentStart(comment_start).into()),
			None => Exit(Reject),
		},
		(1, _) => {
			if let Some(comment_end) = buffer.shift_known_array(b"-->")? {
				Yield(2, Event::CommentEnd(comment_end).into())
			} else if buffer.filled().starts_with(b"--") {
				Error(Error::UnexpectedSequence(b"--"))
			} else {
				match buffer.validate() {
					(valid, Err(error)) if valid.is_empty() => Error(Error::Utf8Error(error)),
					(valid, Ok(())) if valid.is_empty() => return Err(MoreInputRequired::new()),
					//BUG: Detect disallowed characters!
					(valid, _) => {
						if let Some(dashes_at) = valid.find("--") {
							Yield(
								1,
								Event::CommentChunk(
									buffer.shift_validated(dashes_at).expect("unreachable"),
								)
								.into(),
							)
						} else {
							let chunk_len = if valid.ends_with('-') {
								valid.len() - "-".len()
							} else {
								valid.len()
							};
							Yield(
								1,
								Event::CommentChunk(
									buffer.shift_validated(chunk_len).expect("unreachable"),
								)
								.into(),
							)
						}
					}
				}
			}
		}
		(2, _) => Exit(Accept),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [23]
/// Start tokens: `<?xml`
#[instrument(ret(Debug))]
pub(super) fn XMLDecl<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => match buffer.shift_known_array(b"<?xml")? {
			Some(_) => Continue(1),
			None => Exit(Reject),
		},
		(1, _) => Call!(2, VersionInfo),
		(2, Accept) => Call!(21, S),
		(2, Reject) => Error(Error::Expected24VersionInfo),
		(21, Accept) => Call!(3, EncodingDecl_minus_initial_S),
		(21, Reject) => Continue(5),
		(3, Accept) => Call!(31, S),
		(31, Accept) | (3, Reject) => Call!(4, SDDecl_minus_initial_S),
		(4, _) => Call!(5, S),
		(5, _) => match buffer.shift_known_array(b"?>")? {
			Some(_) => Exit(Accept),
			None => Error(Error::ExpectedXMLDeclEnd),
		},

		(
			start_at
			@ (START_AT_VERSION_NUMBER_SINGLE_QUOTE | START_AT_VERSION_NUMBER_DOUBLE_QUOTE),
			_,
		) => CallState!(2, VersionInfo, start_at),

		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [24]
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
		(4 | START_AT_VERSION_NUMBER_SINGLE_QUOTE, _) => Call!(5, VersionNum),
		(5, Accept) => match buffer.shift_known_array(b"'")? {
			Some(_) => Exit(Accept),
			None => Error(Error::ExpectedLiteral(b"'")),
		},
		(6 | START_AT_VERSION_NUMBER_DOUBLE_QUOTE, _) => Call!(7, VersionNum),
		(7, Accept) => match buffer.shift_known_array(b"\"")? {
			Some(_) => Exit(Accept),
			None => Error(Error::ExpectedLiteral(b"\"")),
		},
		(5 | 7, Reject) => unreachable!("should downgrade"),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [25]
/// Start tokens: any
///
/// Never returns `Ok(Exit(Failure))`.
#[instrument(ret(Debug))]
pub(super) fn Eq<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => Call!(1, S),
		(1, _) => match buffer.shift_known_array(b"=")? {
			Some(_) => Continue(2),
			None => Error(Error::ExpectedLiteral(b"=")),
		},
		(2, _) => Call!(3, S),
		(3, _) => Exit(Accept),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [26]
///
/// > \[sic\]. I.e.: Compliant XML 1.0 processors accept documents with other "1." version numbers,
/// > but process them *as if* they were XML 1.0.
#[instrument(ret(Debug))]
fn VersionNum<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => match buffer.shift_known_array(b"1.")? {
			Some(version) => Yield(1, Event::VersionChunk(version).into()),
			None => Error(Error::UnsupportedXmlVersion.into()),
		},
		(1, _) => match buffer.shift_bytes_while(|b| (b'0'..=b'9').contains(&b))? {
			[] => Error(Error::ExpectedDecimalDigit.into()),
			chunk => Yield(2, Event::VersionChunk(chunk).into()),
		},
		(2, _) => match buffer.shift_bytes_while(|b| (b'0'..=b'9').contains(&b))? {
			[] => Exit(Accept),
			chunk => Yield(2, Event::VersionChunk(chunk).into()),
		},
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [27]
#[instrument(ret(Debug))]
fn Misc<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => Call!(1, Comment),
		(1, Accept) => Exit(Accept),
		(1, Reject) => Call!(2, PI),
		(2, Accept) => Exit(Accept),
		(2, Reject) => Call!(3, S),
		(3, either) => Exit(either),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [28]
#[instrument(ret(Debug))]
fn doctypedecl<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => match buffer.shift_known_array(b"<!DOCTYPE")? {
			Some(start) => Yield(1, Event::DoctypedeclStart(start).into()),
			None => Exit(Reject),
		},
		(1, _) => Call!(2, S),
		(2, Accept) => Call!(3, Name),
		(2, Reject) => Error(Error::Expected3Whitespace),
		(3, Accept) => Call!(4, S),
		(3, Reject) => Error(Error::Expected5Name),
		(4, Accept) => Call!(5, ExternalID),
		(4, Reject) => Continue(6),
		(5, _) => Call!(6, S),
		(6, _) => match buffer.shift_known_array(b"[")? {
			Some(_) => Call!(7, intSubset),
			None => Continue(8),
		},
		(7, Accept) => match buffer.shift_known_array(b"]")? {
			Some(_) => Call!(8, S),
			None => Error(Error::ExpectedLiteral(b"]")),
		},
		(7, Reject) => Error(Error::Expected28bIntSubset),
		(8, _) => match buffer.shift_known_array(b">")? {
			Some(end) => Yield(9, Event::DoctypedeclEnd(end).into()),
			None => Error(Error::ExpectedLiteral(b">")),
		},
		(9, _) => Exit(Accept),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [28a]
#[instrument(ret(Debug))]
fn DeclSep<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => Call!(1, PEReference),
		(1, Reject) => Call!(2, S),
		(2, Reject) => Exit(Reject),
		(1 | 2, Accept) => Exit(Accept),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [28b]
#[instrument(ret(Debug))]
fn intSubset<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) | (1 | 2, Accept) => Call!(1, markupdecl),
		(1, Reject) => Call!(2, DeclSep),
		(2, Reject) => Exit(Accept),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [29]
#[instrument(ret(Debug))]
fn markupdecl<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => Call!(1, elementdecl),
		(1, Reject) => Call!(2, AttlistDecl),
		(2, Reject) => Call!(3, EntityDecl),
		(3, Reject) => Call!(4, NotationDecl),
		(4, Reject) => Call!(5, PI),
		(5, Reject) => Call!(6, Comment),
		(6, Reject) => Exit(Reject),
		(1 | 2 | 3 | 4 | 5 | 6, Accept) => Exit(Accept),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [32]
#[instrument(ret(Debug))]
fn SDDecl_minus_initial_S<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	todo!()
}

/// [39], [40], [44]
/// Start tokens: `<`
#[instrument(ret(Debug))]
fn element<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => match buffer.shift_known_array(b"<")? {
			Some(lt) => Yield(1, Event::StartTagStart(lt).into()),
			None => Exit(Reject),
		},
		(1, _) => Call!(2, Name),
		(2, Accept) => Call!(3, S),
		(2, Reject) => Error(Error::Expected5Name),
		(3, Accept) => Call!(4, Attribute),
		(4, Accept) => Call!(3, S),
		(3 | 4, Reject) => match buffer.shift_known_array(b">")? {
			Some(end) => Yield(5, Event::StartTagEnd(end).into()),
			None => Continue(34),
		},
		(34, _) => match buffer.shift_known_array(b"/>")? {
			Some(empty_end) => Yield(8, Event::StartTagEndEmpty(empty_end).into()),
			None => Error(Error::ExpectedStartTagEnd),
		},
		(5, _) => Call!(6, content),
		(6, Accept) => Call!(7, ETag),
		(6, Reject) => unreachable!(),
		(7, Accept) => Exit(Accept),
		(7, Reject) => Error(Error::Expected42ETag),
		(8, _) => Exit(Accept),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [41]
/// Start tokens: See [`Name`].
#[instrument(ret(Debug))]
fn Attribute<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => Call!(1, Name),
		(1, Accept) => Call!(2, Eq),
		(1, Reject) => Exit(Reject),
		(2, Accept) => Call!(3, AttValue),
		(2, Reject) => Error(Error::Expected25Eq),
		(3, Accept) => Exit(Accept),
		(3, Reject) => Error(Error::Expected10AttValue),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [42]
/// Start tokens: `</`
#[instrument(ret(Debug))]
fn ETag<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => match buffer.shift_known_array(b"</")? {
			Some(start) => Yield(1, Event::EndTagStart(start).into()),
			None => Exit(Reject),
		},
		(1, _) => Call!(2, Name),
		(2, Accept) => Call!(3, S),
		(2, Reject) => Error(Error::Expected5Name),
		(3, _) => match buffer.shift_known_array(b">")? {
			Some(end) => Yield(4, Event::EndTagEnd(end).into()),
			None => Error(Error::ExpectedEndTagEnd),
		},
		(4, _) => Exit(Accept),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [43]
#[instrument(ret(Debug))]
fn content<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	todo!()
}

/// [45]
#[instrument(ret(Debug))]
fn elementdecl<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	todo!()
}

/// [52]
#[instrument(ret(Debug))]
fn AttlistDecl<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => match buffer.shift_known_array(b"<!ATTLIST")? {
			Some(start) => Yield(1, Event::AttlistDeclStart(start).into()),
			None => Exit(Reject),
		},
		(1, _) => Call!(2, S),
		(2, Accept) => Call!(3, Name),
		(2, Reject) => Error(Error::Expected3Whitespace),
		(3, Accept) => Call!(4, AttDef),
		(3, Reject) => Error(Error::Expected5Name),
		(4, Accept) => Call!(4, AttDef),
		(4, Reject) => Call!(5, S),
		(5, _) => match buffer.shift_known_array(b">")? {
			Some(end) => Yield(6, Event::AttlistDeclEnd(end).into()),
			None => Error(Error::ExpectedAttlistDeclEnd),
		},
		(6, _) => Exit(Accept),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [53]
#[instrument(ret(Debug))]
fn AttDef<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	todo!()
}

/// [69]
/// Start tokens: `%`
#[instrument(ret(Debug))]
fn PEReference<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => match buffer.shift_known_array(b"%")? {
			Some(start) => Yield(1, Event::PEReferenceStart(start).into()),
			None => Exit(Reject),
		},
		(1, _) => Call!(2, Name),
		(2, Accept) => match buffer.shift_known_array(b";")? {
			Some(end) => Yield(3, Event::PEReferenceEnd(end).into()),
			None => Error(Error::ExpectedLiteral(b";")),
		},
		(2, Reject) => Error(Error::Expected5Name),
		(3, _) => Exit(Accept),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [70]
#[instrument(ret(Debug))]
fn EntityDecl<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	todo!()
}

/// [75]
#[instrument(ret(Debug))]
fn ExternalID<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	todo!()
}

/// [80]
#[instrument(ret(Debug))]
fn EncodingDecl_minus_initial_S<'a>(
	buffer: &mut StrBuf<'a>,
	state: u8,
	ret_val: RetVal,
) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => match buffer.shift_known_array(b"encoding")? {
			Some(_) => Call!(1, Eq),
			None => Exit(Reject),
		},
		(1, Accept) => todo!(),
		(1, Reject) => Error(Error::Expected25Eq),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [82] [75] [83]
///
/// > This is an annoying ambiguous parse when not flattened.
fn NotationDecl<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => match buffer.shift_known_array(b"<!NOTATION")? {
			Some(notation) => Yield(1, Event::NotationDeclStart(notation).into()),
			None => Exit(Reject),
		},
		(1, _) => Call!(2, S),
		(2, Accept) => Call!(3, Name),
		(2, Reject) => Error(Error::Expected3Whitespace),
		(3, Accept) => Call!(4, S),
		(3, Reject) => Error(Error::Expected5Name),
		(4, Accept) => {
			if let Some(system) = buffer.shift_known_array(b"SYSTEM")? {
				Yield(5, Event::SYSTEM(system).into())
			} else if let Some(public) = buffer.shift_known_array(b"PUBLIC")? {
				Yield(todo!(), Event::PUBLIC(public).into())
			} else {
				Error(Error::ExpectedSYSTEMorPUBLIC)
			}
		}
		(4, Reject) => Error(Error::Expected3Whitespace),
		(5, _) => Call!(6, S),
		(6, Accept) => Call!(61, SystemLiteral),
		(6, Reject) => Error(Error::Expected3Whitespace),
		(61, Accept) => Call!(12, S),
		(61, Reject) => Error(Error::Expected11SystemLiteral),
		(7, _) => Call!(8, S),
		(8, Accept) => Call!(9, PubidLiteral),
		(8, Reject) => Error(Error::Expected3Whitespace),
		(9, Accept) => Call!(10, S),
		(9, Reject) => Error(Error::Expected12PubidLiteral),
		(10, Accept) => Call!(11, SystemLiteral),
		(11, Reject) => Continue(12),
		(11, Accept) => Call!(12, S),
		(12, _) => match buffer.shift_known_array(b">")? {
			Some(end) => Yield(13, Event::NotationDeclEnd(end).into()),
			None => Error(Error::ExpectedNotationDeclEnd),
		},
		(13, _) => Exit(Accept),
		_ => unreachable!(),
	}
	.pipe(Ok)
}
