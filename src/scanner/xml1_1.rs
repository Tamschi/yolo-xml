#![allow(clippy::enum_glob_use, non_snake_case, clippy::match_same_arms)]

use super::{
	Error, Event, Event_, MoreInputRequired,
	Next::*,
	NextFnR,
	RetVal::{self, *},
	StringType, TokenizedType,
};
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
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [3]
/// Start tokens: *0x20* | *0x9* | *0xD* | *0xA*
#[instrument(ret(Debug))]
fn S<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => match buffer.shift_known_array(&[0x20])?.is_some()
			|| buffer.shift_known_array(&[0x9])?.is_some()
			|| buffer.shift_known_array(&[0xD])?.is_some()
			|| buffer.shift_known_array(&[0xA])?.is_some()
		{
			true => Continue(1),
			false => Exit(Reject),
		},
		//ON STREAM: Turn into loop.
		(1, _) => match buffer.shift_known_array(&[0x20])?.is_some()
			|| buffer.shift_known_array(&[0x9])?.is_some()
			|| buffer.shift_known_array(&[0xD])?.is_some()
			|| buffer.shift_known_array(&[0xA])?.is_some()
		{
			true => Continue(1),
			false => Exit(Accept),
		},
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

/// [18]
/// Start tokens: `<![CDATA[`
#[instrument(ret(Debug))]
fn CDSect<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => Call!(1, CDStart),
		(1, Accept) => Call!(2, CData),
		(1, Reject) => Exit(Reject),
		(2, Accept) => Call!(3, CDEnd),
		(3, Accept) => Exit(Accept),
		(2 | 3, Reject) => {
			unreachable!("logically unreachable, unless the buffer is manipulated somehow")
		}
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [19]
/// Start tokens: `<![CDATA[`
#[instrument(ret(Debug))]
fn CDStart<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => match buffer.shift_known_array(b"<![CDATA[")? {
			Some(start) => Yield(1, Event::CDStart(start).into()),
			None => Exit(Reject),
		},
		(1, _) => Exit(Accept),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [20]
#[instrument(ret(Debug))]
fn CData<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	todo!()
}

/// [21]
/// Start tokens: `]]>`
#[instrument(ret(Debug))]
fn CDEnd<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => match buffer.shift_known_array(b"]]>")? {
			Some(end) => Yield(1, Event::CDEnd(end).into()),
			None => Exit(Reject),
		},
		(1, _) => Exit(Accept),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [22]
#[instrument(ret(Debug))]
fn prolog<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => Call!(1, XMLDecl),
		(1 | 2, Accept) => Call!(2, Misc),
		(2, Reject) => Call!(3, doctypedecl),
		(3 | 4, Accept) => Call!(4, Misc),
		(3 | 4, Reject) => Exit(Accept),
		(1, Reject) => unreachable!("should downgrade"),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [23]
/// Start tokens: any (because it yields an internal reboot/downgrade event if it doesn't see `<?xml`)
#[instrument(ret(Debug))]
fn XMLDecl<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => match buffer.shift_known_array(b"<?xml")? {
			Some(_) => Continue(1),
			None => Yield(0, Event_::RebootToVersion1_0),
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
		(4, _) => Call!(5, VersionNum),
		(5, Accept) => match buffer.shift_known_array(b"'")? {
			Some(_) => Exit(Accept),
			None => Error(Error::ExpectedLiteral(b"'")),
		},
		(6, _) => Call!(7, VersionNum),
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
fn Eq<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
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
#[instrument(ret(Debug))]
fn VersionNum<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		//BUG: Ensure this is terminated!
		(0, _) => match buffer.shift_known_array(b"1.1")? {
			Some(version) => Yield(1, Event::Version(version).into()),
			None => Yield(0, Event_::DowngradeFrom1_1),
		},
		(1, _) => Exit(Accept),
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

/// [30]
#[instrument(ret(Debug))]
fn extSubset<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => Call!(1, TextDecl),
		(1, _) => Call!(2, extSubsetDecl),
		(2, ret_val) => Exit(ret_val),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [31]
#[instrument(ret(Debug))]
fn extSubsetDecl<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) | (1 | 2 | 3, Accept) => Call!(1, markupdecl),
		(1, Reject) => Call!(2, conditionalSect),
		(2, Reject) => Call!(3, DeclSep),
		(3, Reject) => Exit(Reject),
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

/// [54]
fn AttType<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => Call!(1, StringType),
		(1, Reject) => Call!(2, TokenizedType),
		(2, Reject) => Call!(3, EnumeratedType),
		(1 | 2 | 3, Accept) => Exit(Accept),
		(3, Reject) => Exit(Reject),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [55]
fn StringType<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => match buffer.shift_known_array(b"CDATA")? {
			Some(cdata) => Yield(1, Event::StringType(StringType::CDATA(cdata)).into()),
			None => Exit(Reject),
		},
		(1, _) => Exit(Accept),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [56]
fn TokenizedType<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => {
			// Reordered so shorter strings come after those that contain them.
			if let Some(id_refs) = buffer.shift_known_array(b"IDREFS")? {
				Yield(
					1,
					Event::TokenizedType(TokenizedType::IDREFS(id_refs)).into(),
				)
			} else if let Some(id_ref) = buffer.shift_known_array(b"IDREF")? {
				Yield(1, Event::TokenizedType(TokenizedType::IDREF(id_ref)).into())
			} else if let Some(id) = buffer.shift_known_array(b"ID")? {
				Yield(1, Event::TokenizedType(TokenizedType::ID(id)).into())
			} else if let Some(entity) = buffer.shift_known_array(b"ENTITY")? {
				Yield(
					1,
					Event::TokenizedType(TokenizedType::ENTITY(entity)).into(),
				)
			} else if let Some(entities) = buffer.shift_known_array(b"ENTITIES")? {
				Yield(
					1,
					Event::TokenizedType(TokenizedType::ENTITIES(entities)).into(),
				)
			} else if let Some(nm_tokens) = buffer.shift_known_array(b"NMTOKENS")? {
				Yield(
					1,
					Event::TokenizedType(TokenizedType::NMTOKENS(nm_tokens)).into(),
				)
			} else if let Some(nm_token) = buffer.shift_known_array(b"NMTOKEN")? {
				Yield(
					1,
					Event::TokenizedType(TokenizedType::NMTOKEN(nm_token)).into(),
				)
			} else {
				Exit(Reject)
			}
		}
		(1, _) => Exit(Accept),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [57]
fn EnumeratedType<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => Call!(1, NotationType),
		(1, Reject) => Call!(2, Enumeration),
		(1 | 2, Accept) => Exit(Accept),
		(2, Reject) => Exit(Reject),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [58]
fn NotationType<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	todo!()
}

/// [59]
fn Enumeration<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	todo!()
}

/// [61]
#[instrument(ret(Debug))]
fn conditionalSect<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	todo!()
}

/// [66]
#[instrument(ret(Debug))]
fn CharRef<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	todo!()
}

/// [67]
/// Start tokens: `&`
///
/// > This needs to try [`CharRef`] (which starts with `&#` or `&#x`) before [`EntityRef`] (which starts with just `&`).
#[instrument(ret(Debug))]
fn Reference<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => Call!(1, CharRef),
		(1, Reject) => Call!(2, EntityRef),
		(2, Reject) => Exit(Reject),
		(1 | 2, Accept) => Exit(Accept),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [68]
/// Start tokens: `&`
#[instrument(ret(Debug))]
fn EntityRef<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => match buffer.shift_known_array(b"&")? {
			Some(start) => Yield(1, Event::EntityRefStart(start).into()),
			None => Exit(Reject),
		},
		(1, _) => Call!(2, Name),
		(2, Accept) => match buffer.shift_known_array(b";")? {
			Some(end) => Yield(3, Event::EntityRefEnd(end).into()),
			None => Error(Error::ExpectedLiteral(b";")),
		},
		(2, Reject) => Error(Error::Expected5Name),
		(3, _) => Exit(Accept),
		_ => unreachable!(),
	}
	.pipe(Ok)
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

/// [77]
#[instrument(ret(Debug))]
fn TextDecl<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
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

/// [82]
#[instrument(ret(Debug))]
fn NotationDecl<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	todo!()
}
