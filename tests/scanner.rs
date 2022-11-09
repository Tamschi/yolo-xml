use std::{mem::MaybeUninit, sync::Once};
use tracing::{info_span, subscriber};
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, Registry};
use tracing_tree::HierarchicalLayer;
use yolo_xml::{
	buffer::StrBuf,
	scanner::{Event, Scanner},
};

#[test]
fn xml_declaration() {
	setup();

	expect_events(
		"<?xml version=\"1.1\"?>",
		&[Event::VersionChunk(&mut b"1.1".to_owned())],
	);
}

#[test]
fn downgrade_1() {
	setup();

	expect_events(" ", &[]);
}

#[test]
fn downgrade_2() {
	setup();

	expect_events(
		"<?xml version=\"1.0\"?>",
		&[
			Event::VersionChunk(&mut b"1.".to_owned()),
			Event::VersionChunk(&mut b"0".to_owned()),
		],
	);
}

#[test]
fn downgrade_3() {
	setup();

	expect_events(
		"<?xml version=\"1.12345\"?>",
		&[
			Event::VersionChunk(&mut b"1.".to_owned()),
			Event::VersionChunk(&mut b"12345".to_owned()),
		],
	);
}

#[test]
fn downgrade_4() {
	setup();

	expect_events(
		"<?xml version=\"1.77777\"?>",
		&[
			Event::VersionChunk(&mut b"1.".to_owned()),
			Event::VersionChunk(&mut b"77777".to_owned()),
		],
	);
}

#[test]
fn empty() {
	setup();

	expect_events(
		"<empty />",
		&[
			Event::StartTagStart(&mut b"<".to_owned()),
			Event::NameChunk(&mut "empty".to_owned()),
			Event::StartTagEndEmpty(&mut b"/>".to_owned()),
		],
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
		let _span = info_span!("Expecting event", expected = ?expected).entered();
		assert_eq!(
			scanner.resume(&mut buffer).unwrap().unwrap().unwrap(),
			*expected
		);
	}
	{
		let _span = info_span!("Expecting needs more data").entered();
		scanner.resume(&mut buffer).unwrap_err();
	}
	assert_eq!(buffer.filled().len(), 0);
}

static SETUP_ONCE: Once = Once::new();
fn setup() {
	SETUP_ONCE.call_once(|| {
		let subscriber =
			Registry::default().with(Box::new(HierarchicalLayer::new(2).with_indent_lines(true)));
		subscriber::set_global_default(subscriber).unwrap();
	});
}
