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
