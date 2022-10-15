use crate::buffer::{Indeterminate, StrBuf, Utf8Error};
use tap::Pipe;

type NextFn = for<'a> fn(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a>;
type NextFnR<'a> = Result<Next<'a>, MoreInputRequired>;
enum Next<'a> {
	Exit(RetVal),
	Call(u8, NextFn),
	Yield(u8, Event_<'a>),
	Continue(u8),
	Error(Error),
}
use Next::*;

enum RetVal {
	Accept,
	Reject,
}
use RetVal::*;

pub struct Scanner {
	depth_limit: usize,
	states: Vec<u8>,
	call_stack: Vec<NextFn>,
}

#[derive(Debug)]
pub enum ScannerError {
	DepthLimitExceeded,
	XmlError(Error),
}

#[derive(Debug)]
pub struct MoreInputRequired {
	_private: (),
}
impl MoreInputRequired {
	pub(crate) fn new() -> Self {
		Self { _private: () }
	}
}
impl From<Indeterminate> for MoreInputRequired {
	fn from(_: Indeterminate) -> Self {
		Self::new()
	}
}

impl Scanner {
	pub fn new(depth_limit: usize) -> Self {
		Self {
			depth_limit,
			states: vec![0],
			call_stack: vec![document],
		}
	}

	pub fn resume<'a>(
		&mut self,
		buffer: &mut StrBuf<'a>,
	) -> Result<Result<Option<Event<'a>>, ScannerError>, MoreInputRequired> {
		let mut last_ret_val = Accept;
		loop {
			let next = self
				.call_stack
				.last()
				.expect("Called resume while the call stack was empty.")(
				buffer,
				*self.states.last().expect("unreachable"),
				last_ret_val,
			)?;
			// Not strictly necessary, but in case there's a bug this makes it reliably work/not work.
			last_ret_val = Accept;
			match next {
				Exit(ret_val) => {
					last_ret_val = ret_val;
					self.states.pop();
					self.call_stack.pop();
				}
				Call(state, callee) => {
					if self.states.len() >= self.depth_limit {
						break Err(ScannerError::DepthLimitExceeded);
					}
					*self.states.last_mut().expect("unreachable") = state;
					self.states.push(0);
					self.call_stack.push(callee);
				}
				Yield(state, internal_event) => {
					*self.states.last_mut().expect("unreachable") = state;
					match internal_event {
						Event_::Public(event) => return Ok(Ok(Some(event))),
						Event_::RebootToVersion1_0 => todo!(),
						Event_::DowngradeFrom1_1 => todo!(),
					}
				}
				Continue(state) => *self.states.last_mut().expect("unreachable") = state,
				Next::Error(error) => break Err(ScannerError::XmlError(error)),
			}
		}
		.pipe(Ok)
	}
}

/// [1]
fn document<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => Call(1, prolog),
		(1, Accept) => Call(2, element),
		(2 | 3, Accept) => Call(3, Misc),
		(3, Reject) => Exit(Accept),
		(1, Reject) => Error(Error::Expected22Prolog),
		(2, Reject) => Error(Error::Expected39Element),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [3]
/// Start tokens: *0x20* | *0x9* | *0xD* | *0xA*
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
fn Name<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	todo!()
}

/// [10]
fn AttValue<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	todo!()
}

/// [15]
/// Start tokens: `<!--`
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
fn PI<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => match buffer.shift_known_array(b"<?")? {
			Some(start) => Yield(1, Event::PIStart(start).into()),
			None => Exit(Reject),
		},
		(1, _) => Call(2, PITarget),
		(2, Accept) => Call(3, S),
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
				Continue(41)
			}
		}
		(41, _) => match buffer.validate() {
			(valid, Err(error)) if valid.is_empty() => Error(Error::Utf8Error(error)),
			(valid, Ok(())) if valid.is_empty() => return Err(MoreInputRequired::new()),
			//BUG: Detect disallowed characters!
			(_valid, _) => Continue(42),
		},
		(42, _) => Yield(
			4,
			Event::PIChunk(
				buffer
					.shift_validated(if buffer.validated().ends_with("?") {
						buffer.validated().len() - 1
					} else {
						buffer.validated().len()
					})
					.expect("unreachable"),
			)
			.into(),
		),
		(5, _) => Exit(Accept),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [17]
fn PITarget<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	todo!()
}

/// [18]
/// Start tokens: `<![CDATA[`
fn CDSect<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => Call(1, CDStart),
		(1, Accept) => Call(2, CData),
		(1, Reject) => Exit(Reject),
		(2, Accept) => Call(3, CDEnd),
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
fn CData<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	todo!()
}

/// [21]
/// Start tokens: `]]>`
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
fn prolog<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => Call(1, XMLDecl),
		(1 | 2, Accept) => Call(2, Misc),
		(2, Reject) => Call(3, doctypedecl),
		(3 | 4, Accept) => Call(4, Misc),
		(3 | 4, Reject) => Exit(Accept),
		(1, Reject) => unreachable!("should downgrade"),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [23]
/// Start tokens: any (because it yields an internal reboot/downgrade event if it doesn't see `<?xml`)
fn XMLDecl<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => match buffer.shift_known_array(b"<?xml")? {
			Some(_) => Continue(1),
			None => Yield(0, Event_::RebootToVersion1_0),
		},
		(1, _) => Call(2, VersionInfo),
		(2, Accept) => Call(1, S),
		(2, Reject) => Error(Error::Expected24VersionInfo),
		(21, Accept) => Call(3, EncodingDecl_minus_initial_S),
		//TODO
		(3, _) => Call(4, SDDecl),
		(4, _) => Call(5, S),
		(5, _) => match buffer.shift_known_array(b"?>")? {
			Some(_) => Accept,
			None => Reject,
		}
		.pipe(Exit),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [24]
fn VersionInfo<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => Call(1, S),
		(1, Accept) => match buffer.shift_known_array(b"version")? {
			Some(_) => Continue(2),
			None => Error(Error::ExpectedLiteral(b"version")),
		},
		(1, Reject) => Exit(Reject),
		(2, _) => Call(3, Eq),
		(3, Accept) => match buffer.shift_known_array(b"'")? {
			Some(_) => Continue(4),
			None => match buffer.shift_known_array(b"\"")? {
				Some(_) => Continue(6),
				None => Error(Error::ExpectedQuote),
			},
		},
		(3, Reject) => unreachable!("`Eq` shouldn't fail."),
		(4, _) => Call(5, VersionNum),
		(5, Accept) => match buffer.shift_known_array(b"'")? {
			Some(_) => Exit(Accept),
			None => Error(Error::ExpectedLiteral(b"'")),
		},
		(6, _) => Call(7, VersionNum),
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
fn Eq<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => Call(1, S),
		(1, _) => match buffer.shift_known_array(b"=")? {
			Some(_) => Continue(2),
			None => Error(Error::ExpectedLiteral(b"=")),
		},
		(2, _) => Call(3, S),
		(3, _) => Exit(Accept),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [26]
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
fn Misc<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => Call(1, Comment),
		(1, Accept) => Exit(Accept),
		(1, Reject) => Call(2, PI),
		(2, Accept) => Exit(Accept),
		(2, Reject) => Call(3, S),
		(3, either) => Exit(either),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [28]
fn doctypedecl<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => match buffer.shift_known_array(b"<!DOCTYPE")? {
			Some(start) => Yield(1, Event::DoctypedeclStart(start).into()),
			None => Exit(Reject),
		},
		(1, _) => Call(2, S),
		(2, Accept) => Call(3, Name),
		(2, Reject) => Error(Error::Expected3Whitespace),
		(3, Accept) => Call(4, S),
		(3, Reject) => Error(Error::Expected5Name),
		(4, Accept) => Call(5, ExternalID),
		(4, Reject) => Continue(6),
		(5, _) => Call(6, S),
		(6, _) => match buffer.shift_known_array(b"[")? {
			Some(_) => Call(7, intSubset),
			None => Continue(8),
		},
		(7, Accept) => match buffer.shift_known_array(b"]")? {
			Some(_) => Call(8, S),
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
fn DeclSep<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => Call(1, PEReference),
		(1, Reject) => Call(2, S),
		(2, Reject) => Exit(Reject),
		(1 | 2, Accept) => Exit(Accept),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [28b]
fn intSubset<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) | (1 | 2, Accept) => Call(1, markupdecl),
		(1, Reject) => Call(2, DeclSep),
		(2, Reject) => Exit(Accept),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [29]
fn markupdecl<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => Call(1, elementdecl),
		(1, Reject) => Call(2, AttlistDecl),
		(2, Reject) => Call(3, EntityDecl),
		(3, Reject) => Call(4, NotationDecl),
		(4, Reject) => Call(5, PI),
		(5, Reject) => Call(6, Comment),
		(6, Reject) => Exit(Reject),
		(1 | 2 | 3 | 4 | 5 | 6, Accept) => Exit(Accept),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [30]
fn extSubset<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => Call(1, TextDecl),
		(1, _) => Call(2, extSubsetDecl),
		(2, ret_val) => Exit(ret_val),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [31]
fn extSubsetDecl<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) | (1 | 2 | 3, Accept) => Call(1, markupdecl),
		(1, Reject) => Call(2, conditionalSect),
		(2, Reject) => Call(3, DeclSep),
		(3, Reject) => Exit(Reject),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [32]
fn SDDecl<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	todo!()
}

/// [39], [40], [44]
/// Start tokens: `<`
fn element<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => match buffer.shift_known_array(b"<")? {
			Some(lt) => Yield(1, Event::StartTagStart(lt).into()),
			None => Exit(Reject),
		},
		(1, _) => Call(2, Name),
		(2, Accept) => Call(3, S),
		(2, Reject) => Error(Error::Expected5Name),
		(3, Accept) => Call(4, Attribute),
		(4, Accept) => Call(3, S),
		(3 | 4, Reject) => match buffer.shift_known_array(b">")? {
			Some(end) => Yield(5, Event::StartTagEnd(end).into()),
			None => Continue(34),
		},
		(34, _) => match buffer.shift_known_array(b"/>")? {
			Some(empty_end) => Yield(8, Event::StartTagEndEmpty(empty_end).into()),
			None => Error(Error::ExpectedStartTagEnd),
		},
		(5, _) => Call(6, content),
		(6, Accept) => Call(7, ETag),
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
fn Attribute<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => Call(1, Name),
		(1, Accept) => Call(2, Eq),
		(1, Reject) => Exit(Reject),
		(2, Accept) => Call(3, AttValue),
		(2, Reject) => Error(Error::Expected25Eq),
		(3, Accept) => Exit(Accept),
		(3, Reject) => Error(Error::Expected10AttValue),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [42]
/// Start tokens: `</`
fn ETag<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => match buffer.shift_known_array(b"</")? {
			Some(start) => Yield(1, Event::EndTagStart(start).into()),
			None => Exit(Reject),
		},
		(1, _) => Call(2, Name),
		(2, Accept) => Call(3, S),
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
fn content<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	todo!()
}

/// [45]
fn elementdecl<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	todo!()
}

/// [52]
fn AttlistDecl<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => match buffer.shift_known_array(b"<!ATTLIST")? {
			Some(start) => Yield(1, Event::AttlistDeclStart(start).into()),
			None => Exit(Reject),
		},
		(1, _) => Call(2, S),
		(2, Accept) => Call(3, Name),
		(2, Reject) => Error(Error::Expected3Whitespace),
		(3, Accept) => Call(4, AttDef),
		(3, Reject) => Error(Error::Expected5Name),
		(4, Accept) => Call(4, AttDef),
		(4, Reject) => Call(5, S),
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
fn AttDef<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	todo!()
}

/// [61]
fn conditionalSect<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	todo!()
}

/// [66]
fn CharRef<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	todo!()
}

/// [67]
/// Start tokens: `&`
///
/// > This needs to try [`CharRef`] (which starts with `&#` or `&#x`) before [`EntityRef`] (which starts with just `&`).
fn Reference<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => Call(1, CharRef),
		(1, Reject) => Call(2, EntityRef),
		(2, Reject) => Exit(Reject),
		(1 | 2, Accept) => Exit(Accept),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [68]
/// Start tokens: `&`
fn EntityRef<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => match buffer.shift_known_array(b"&")? {
			Some(start) => Yield(1, Event::EntityRefStart(start).into()),
			None => Exit(Reject),
		},
		(1, _) => Call(2, Name),
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
fn PEReference<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => match buffer.shift_known_array(b"%")? {
			Some(start) => Yield(1, Event::PEReferenceStart(start).into()),
			None => Exit(Reject),
		},
		(1, _) => Call(2, Name),
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
fn EntityDecl<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	todo!()
}

/// [75]
fn ExternalID<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	todo!()
}

/// [77]
fn TextDecl<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	todo!()
}

/// [80]
fn EncodingDecl_minus_initial_S<'a>(
	buffer: &mut StrBuf<'a>,
	state: u8,
	ret_val: RetVal,
) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => match buffer.shift_known_array(b"encoding")? {
			Some(_) => Call(1, Eq),
			None => Exit(Reject),
		},
		(1, Accept) => todo!(),
		(1, Reject) => Error(Error::Expected25Eq),
		_ => unreachable!(),
	}
	.pipe(Ok)
}

/// [82]
fn NotationDecl<'a>(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	todo!()
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

#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Event<'a> {
	Version(&'a mut [u8]),
	CommentStart(&'a mut [u8; 4]),
	CommentEnd(&'a mut [u8; 3]),
	CommentChunk(&'a mut str),
	StartTagStart(&'a mut [u8; 1]),
	StartTagEndEmpty(&'a mut [u8; 2]),
	StartTagEnd(&'a mut [u8; 1]),
	EndTagStart(&'a mut [u8; 2]),
	EndTagEnd(&'a mut [u8; 1]),
	PIStart(&'a mut [u8; 2]),
	PIEnd(&'a mut [u8; 2]),
	PIChunk(&'a mut str),
	DoctypedeclStart(&'a mut [u8; 9]),
	DoctypedeclEnd(&'a mut [u8; 1]),
	PEReferenceStart(&'a mut [u8; 1]),
	PEReferenceEnd(&'a mut [u8; 1]),
	EntityRefStart(&'a mut [u8; 1]),
	EntityRefEnd(&'a mut [u8; 1]),
	CDStart(&'a mut [u8; 9]),
	CDEnd(&'a mut [u8; 3]),
	AttlistDeclStart(&'a mut [u8; 9]),
	AttlistDeclEnd(&'a mut [u8; 1]),
}

#[derive(Debug)]
pub enum Error {
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
	ExpectedEndTagEnd,
	Expected25Eq,
	Expected10AttValue,
	Expected17PITarget,
	ExpectedWhitespaceOrPIEnd,
	Expected28bIntSubset,
	ExpectedAttlistDeclEnd,
}
