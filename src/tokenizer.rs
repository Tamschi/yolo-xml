use crate::buffer::{OutOfBoundsError, StrBuf};
use core::mem::{ManuallyDrop, MaybeUninit};
use std::{ops::Deref, ptr::addr_of};

type NextFn = for<'a> fn(buffer: &'a mut StrBuf, state: u8, ret_val: RetVal) -> NextFnR<'a>;
type NextFnR<'a> = Result<Result<Next<'a>, Error>, OutOfBoundsError>;
enum Next<'a> {
	Exit(RetVal),
	Call(u8, NextFn),
	Yield(u8, Event<'a>),
	Continue(u8),
}
use Next::*;

enum RetVal {
	Success,
	Failure,
}
use tap::Pipe;
use RetVal::*;

/// [1]
fn document<'a>(buffer: &'a mut StrBuf, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => Call(1, prolog),
		(1, Success) => Call(2, element),
		(2 | 3, Success) => Call(3, Misc),
		(3, Failure) => Exit(Success),
		(1 | 2, Failure) => Exit(Failure),
		_ => unreachable!(),
	}
	.pipe(Ok)
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
	}
	.pipe(Ok)
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
		(1, Failure) => Exit(Failure),
		_ => unreachable!(),
	}
	.pipe(Ok)
	.pipe(Ok)
}

/// [23]
fn XMLDecl<'a>(buffer: &'a mut StrBuf, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => match buffer.shift_known_array(b"<?xml")? {
			Some(_) => Continue(1),
			None => Yield(0, Event::DowngradeFrom1_1),
		},
		(1, _) => Call(2, VersionInfo),
		(2, Success) => Call(3, EncodingDecl),
		(3, _) => Call(4, SDDecl),
		(4, _) => Call(5, S),
		(5, _) => match buffer.shift_known_array(b"?>")? {
			Some(_) => Success,
			None => Failure,
		}
		.pipe(Exit),
		(2, Failure) => Exit(Failure),
		_ => unreachable!(),
	}
	.pipe(Ok)
	.pipe(Ok)
}

/// [24]
fn VersionInfo<'a>(buffer: &'a mut StrBuf, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => Call(1, S),
		(1, Success) => match buffer.shift_known_array(b"version")? {
			Some(_) => Continue(2),
			None => Exit(Failure),
		},
		(2, _) => Call(3, Eq),
		(3, Success) => match buffer.shift_known_array(b"'")? {
			Some(_) => Continue(4),
			None => match buffer.shift_known_array(b"\"")? {
				Some(_) => Continue(6),
				None => Exit(Failure),
			},
		},
		(4, _) => Call(5, VersionNum),
		(5, Success) => match buffer.shift_known_array(b"'")? {
			Some(_) => Exit(Success),
			None => Exit(Failure),
		},
		(6, _) => Call(7, VersionNum),
		(7, Success) => match buffer.shift_known_array(b"\"")? {
			Some(_) => Exit(Success),
			None => Exit(Failure),
		},
		(1 | 3 | 5 | 7, Failure) => Exit(Failure),
		_ => unreachable!(),
	}
	.pipe(Ok)
	.pipe(Ok)
}

/// [25]
fn Eq<'a>(buffer: &'a mut StrBuf, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => Call(1, S),
		(1, _) => match buffer.shift_known_array(b"=")? {
			Some(_) => Continue(2),
			None => Exit(Failure),
		},
		(2, _) => Call(3, S),
		(3, _) => Exit(Success),
		_ => unreachable!(),
	}
	.pipe(Ok)
	.pipe(Ok)
}

/// [26]
fn VersionNum<'a>(buffer: &'a mut StrBuf, state: u8, ret_val: RetVal) -> NextFnR<'a> {
	match (state, ret_val) {
		(0, _) => match buffer.shift_known_array(b"1.1")? {
			Some(version) => Yield(1, Event::Version(version)),
			None => Yield(0, Event::DowngradeFrom1_1),
		}
		.pipe(Ok)
		.pipe(Ok),
	}
}

#[non_exhaustive]
enum Event<'a> {
	DowngradeFrom1_1,
	Version(&'a mut [u8]),
}

enum Error {}
