use std::mem::MaybeUninit;

use yolo_xml::{
	buffer::StrBuf,
	scanner::{Event, Scanner},
};

#[test]
fn xml_declaration() {
	expect_events(
		"<?xml version=\"1.1\">",
		&[Event::Version(&mut b"1.1".to_owned())],
	);
}

fn expect_events(input: impl AsRef<[u8]>, events: &[Event]) {
	let mut buffer = Vec::from_iter(input.as_ref().iter().copied().map(MaybeUninit::new));
	let mut buffer = StrBuf::new(buffer.as_mut_slice());
	unsafe {
		buffer.assume_filled_n_remaining(buffer.remaining_len());
	}

	let mut scanner = Scanner::new(10);
	for expected in events {
		assert_eq!(
			scanner.resume(&mut buffer).unwrap().unwrap().unwrap(),
			*expected
		);
	}
	scanner.resume(&mut buffer).unwrap_err();
}
