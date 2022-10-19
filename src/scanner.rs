use crate::buffer::{Indeterminate, StrBuf, Utf8Error};
use std::fmt::Debug;
use tap::Pipe;
use tracing::instrument;

mod xml1_1;

type NextFn = for<'a> fn(buffer: &mut StrBuf<'a>, state: u8, ret_val: RetVal) -> NextFnR<'a>;
type NextFnR<'a> = Result<Next<'a>, MoreInputRequired>;

enum Next<'a> {
	Exit(RetVal),
	Call(u8, NextFn, #[cfg(debug_assertions)] &'static str),
	Yield(u8, Event_<'a>),
	Continue(u8),
	Error(Error),
}
#[allow(clippy::enum_glob_use)]
use Next::*;
impl Debug for Next<'_> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Exit(ret_val) => f.debug_tuple("Exit").field(ret_val).finish(),
			#[cfg(not(debug_assertions))]
			Self::Call(state, callee) => f
				.debug_tuple("Call")
				.field(state)
				.field(&(*callee as usize))
				.finish(),
			#[cfg(debug_assertions)]
			Self::Call(state, _callee, name) => f.debug_tuple("Call").field(state).field(name).finish(),
			Self::Yield(state, event) => f.debug_tuple("Yield").field(state).field(event).finish(),
			Self::Continue(state) => f.debug_tuple("Continue").field(state).finish(),
			Self::Error(error) => f.debug_tuple("Error").field(error).finish(),
		}
	}
}

#[derive(Debug)]
enum RetVal {
	Accept,
	Reject,
}

pub struct Scanner {
	depth_limit: usize,
	states: Vec<u8>,
	call_stack: Vec<NextFn>,
}
impl Debug for Scanner {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Scanner")
			.field("depth_limit", &self.depth_limit)
			.field("states", &self.states)
			// .field("call_stack", &self.call_stack)
			.finish()
	}
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
			call_stack: vec![xml1_1::document],
		}
	}

	//ON STREAM: Return an error if the buffer is clogged!
	#[instrument(ret(Debug))]
	pub fn resume<'a>(
		&mut self,
		buffer: &mut StrBuf<'a>,
	) -> Result<Result<Option<Event<'a>>, ScannerError>, MoreInputRequired> {
		let mut last_ret_val = RetVal::Accept;
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
			last_ret_val = RetVal::Accept;
			match next {
				Exit(ret_val) => {
					last_ret_val = ret_val;
					self.states.pop();
					self.call_stack.pop();
				}
				#[cfg(not(debug_assertions))]
				Call(state, callee) => {
					if self.states.len() >= self.depth_limit {
						break Err(ScannerError::DepthLimitExceeded);
					}
					*self.states.last_mut().expect("unreachable") = state;
					self.states.push(0);
					self.call_stack.push(callee);
				}
				#[cfg(debug_assertions)]
				Call(state, callee, _name) => {
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

#[derive(Debug)]
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
	StringType(StringType<'a>),
	TokenizedType(TokenizedType<'a>),
	NotationDeclStart(&'a mut [u8; 10]),
	SYSTEM(&'a mut [u8; 6]),
	PUBLIC(&'a mut [u8; 6]),
	NotationDeclEnd(&'a mut [u8; 1]),
}

#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum StringType<'a> {
	CDATA(&'a mut [u8; 5]),
}

#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TokenizedType<'a> {
	ID(&'a mut [u8; 2]),
	IDREF(&'a mut [u8; 5]),
	IDREFS(&'a mut [u8; 6]),
	ENTITY(&'a mut [u8; 6]),
	ENTITIES(&'a mut [u8; 8]),
	NMTOKEN(&'a mut [u8; 7]),
	NMTOKENS(&'a mut [u8; 8]),
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
	ExpectedXMLDeclEnd,
	ExpectedSYSTEMorPUBLIC,
	Expected12PubidLiteral,
	Expected11SystemLiteral,
	ExpectedNotationDeclEnd,
}
