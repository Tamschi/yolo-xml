use crate::buffer::StrBuf;
use core::mem::{ManuallyDrop, MaybeUninit};

pub struct XmlTokenizer {
	buffer: ManuallyDrop<StrBuf<'static>>,
	memory: [MaybeUninit<u8>],
}
