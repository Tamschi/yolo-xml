use crate::buffer::{OutOfBoundsError, StrBuf, Utf8Error};
use tap::Pipe;

type NextFn = for<'a> fn(buffer: &'a mut StrBuf, state: u8, ret_val: RetVal) -> NextFnR<'a>;
type NextFnR<'a> = Result<Next<'a>, OutOfBoundsError>;
enum Next<'a> {
	Exit(RetVal),
	Call(u8, NextFn),
	Yield(u8, Event_<'a>),
	Continue(u8),
	Error(Error),
}
use Next::*;

enum RetVal {
	Success,
	Failure,
}
use RetVal::*;

/// [1]
fn document<'a>(buffer: &'a mut StrBuf, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => Call(1, prolog),
		(1, Success) => Call(2, element),
		(2 | 3, Success) => Call(3, Misc),
		(3, Failure) => Exit(Success),
		(1, Failure) => Error(Error::Expected22Prolog),
		(2, Failure) => Error(Error::Expected39Element),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [3]
fn S<'a>(buffer: &'a mut StrBuf, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => match buffer
			.shift_known_array(&[0x20])
			.transpose()
			.or_else(|| buffer.shift_known_array(&[0x9]).transpose())
			.or_else(|| buffer.shift_known_array(&[0xD]).transpose())
			.or_else(|| buffer.shift_known_array(&[0xA]).transpose())
			.transpose()?
		{
			Some(_) => Continue(1),
			None => Exit(Failure),
		},
		(1, _) => match buffer
			.shift_known_array(&[0x20])
			.transpose()
			.or_else(|| buffer.shift_known_array(&[0x9]).transpose())
			.or_else(|| buffer.shift_known_array(&[0xD]).transpose())
			.or_else(|| buffer.shift_known_array(&[0xA]).transpose())
			.transpose()?
		{
			Some(_) => Continue(1),
			None => Exit(Success),
		},
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [15]
fn Comment<'a>(buffer: &'a mut StrBuf, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => match buffer.shift_known_array(b"<!--")? {
			Some(comment_start) => Yield(1, Event::CommentStart(comment_start).into()),
			None => Exit(Failure),
		},
		(1, _) => {
			if let Some(comment_end) = buffer.shift_known_array(b"-->")? {
				Yield(2, Event::CommentEnd(comment_end).into())
			} else if buffer.filled().starts_with(b"--") {
				Error(Error::UnexpectedSequence(b"--"))
			} else {
				match buffer.validate() {
					(valid, Err(error @ Utf8Error)) if valid.is_empty() => {
						Error(Error::Utf8Error(error))
					}
					(valid, Ok(())) if valid.is_empty() => return Err(OutOfBoundsError::new()),
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
							Yield(
								1,
								Event::CommentChunk(
									buffer
										.shift_validated(if valid.ends_with("-") {
											valid.len() - "-".len()
										} else {
											valid.len()
										})
										.expect("unreachable"),
								)
								.into(),
							)
						}
					}
				}
			}
		}
		(2, _) => Exit(Success),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [22]
fn prolog<'a>(buffer: &'a mut StrBuf, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => Call(1, XMLDecl),
		(1 | 2, Success) => Call(2, Misc),
		(2, Failure) => Call(3, doctypedecl),
		(3 | 4, Success) => Call(4, Misc),
		(3 | 4, Failure) => Exit(Success),
		(1, Failure) => unreachable!("should downgrade"),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [23]
fn XMLDecl<'a>(buffer: &'a mut StrBuf, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => match buffer.shift_known_array(b"<?xml")? {
			Some(_) => Continue(1),
			None => Yield(0, Event_::Reboot_Version_1_0),
		},
		(1, _) => Call(2, VersionInfo),
		(2, Success) => Call(3, EncodingDecl),
		(2, Failure) => Error(Error::Expected24VersionInfo),
		(3, _) => Call(4, SDDecl),
		(4, _) => Call(5, S),
		(5, _) => match buffer.shift_known_array(b"?>")? {
			Some(_) => Success,
			None => Failure,
		}
		.pipe(Exit),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [24]
fn VersionInfo<'a>(buffer: &'a mut StrBuf, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => Call(1, S),
		(1, Success) => match buffer.shift_known_array(b"version")? {
			Some(_) => Continue(2),
			None => Error(Error::ExpectedLiteral(b"version")),
		},
		(1, Failure) => Exit(Failure),
		(2, _) => Call(3, Eq),
		(3, Success) => match buffer.shift_known_array(b"'")? {
			Some(_) => Continue(4),
			None => match buffer.shift_known_array(b"\"")? {
				Some(_) => Continue(6),
				None => Error(Error::ExpectedQuote),
			},
		},
		(3, Failure) => unreachable!("`Eq` shouldn't fail."),
		(4, _) => Call(5, VersionNum),
		(5, Success) => match buffer.shift_known_array(b"'")? {
			Some(_) => Exit(Success),
			None => Error(Error::ExpectedLiteral(b"'")),
		},
		(6, _) => Call(7, VersionNum),
		(7, Success) => match buffer.shift_known_array(b"\"")? {
			Some(_) => Exit(Success),
			None => Error(Error::ExpectedLiteral(b"\"")),
		},
		(5 | 7, Failure) => unreachable!("should downgrade"),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [25]
///
/// Never returns `Ok(Exit(Failure))`.
fn Eq<'a>(buffer: &'a mut StrBuf, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => Call(1, S),
		(1, _) => match buffer.shift_known_array(b"=")? {
			Some(_) => Continue(2),
			None => Error(Error::ExpectedLiteral(b"=")),
		},
		(2, _) => Call(3, S),
		(3, _) => Exit(Success),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [26]
fn VersionNum<'a>(buffer: &'a mut StrBuf, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => match buffer.shift_known_array(b"1.1")? {
			Some(version) => Yield(1, Event::Version(version).into()),
			None => Yield(0, Event_::DowngradeFrom1_1),
		},
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [27]
fn Misc<'a>(buffer: &'a mut StrBuf, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => Call(1, Comment),
		(1, Success) => Exit(Success),
		(1, Failure) => Call(2, PI),
		(2, Success) => Exit(Success),
		(2, Failure) => Call(3, S),
		(3, either) => Exit(either),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [39], [40], [44]
fn element<'a>(buffer: &'a mut StrBuf, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => match buffer.shift_known_array(b"<")? {
			Some(lt) => Yield(1, Event::TagStartStart(lt).into()),
			None => Exit(Failure),
		},
		(1, _) => Call(2, Name),
		(2, Success) => Call(3, S),
		(2, Failure) => Error(Error::Expected5Name),
		(3, Success) => Call(Attribute, 4),
		(4, Success) => Call(3, S),
		(3 | 4, Failure) => match buffer.shift_known_array(b">")? {
			Some(end) => Yield(5, Event::StartTagEnd(end).into()),
			None => match buffer.shift_known_array(b"/>")? {
				Some(empty_end) => Yield(8, Event::StartTagEndEmpty(empty_end).into()),
				None => Error(Error::ExpectedStartTagEnd),
			},
		},
		(5, _) => Call(6, Content),
		(6, Success) => Call(7, ETag),
		(6, Failure) => unreachable!(),
		(7, Success) => Exit(Success),
		(7, Failure) => Error(Error::Expected42ETag),
		(8, _) => Exit(Success),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

enum Event_<'a> {
	Public(Event<'a>),
	RebootToVersion1_0,
	DowngradeFrom1_1,
}
impl<'a> From<Event<'a>> for Event_<'a> {
	fn from(event: Event<'a>) -> Self {
		Self::Public(event)
	}
}

#[non_exhaustive]
pub enum Event<'a> {
	Version(&'a mut [u8]),
	CommentStart(&'a mut [u8; 4]),
	CommentEnd(&'a mut [u8; 3]),
	CommentChunk(&'a mut str),
	TagStartStart(&'a mut [u8; 1]),
	StartTagEndEmpty(&mut [u8; 2]),
	StartTagEnd(&mut [u8; 1]),
}

enum Error {
	ExpectedLiteral(&'static [u8]),
	ExpectedQuote,
	Expected3Whitespace,
	Expected22Prolog,
	Expected24VersionInfo,
	Expected39Element,
	Utf8Error(Utf8Error),
	UnexpectedSequence(&'static [u8]),
	Expected5Name,
	ExpectedStartTagEnd,
	Expected42ETag,
}
