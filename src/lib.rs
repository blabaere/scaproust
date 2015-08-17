#![crate_name = "scaproust"]

#![feature(drain)]
#![feature(fnbox)]
#![feature(unboxed_closures)]
#![feature(split_off)]
#![feature(vec_push_all)]
#![feature(duration)]
#![feature(time)]

#[macro_use] extern crate log;
extern crate byteorder;
extern crate mio;
extern crate time;

mod global;
mod event_loop_msg;
mod session;
mod session_impl;
mod socket;
mod socket_impl;
mod protocol;
mod transport;
mod pipe;
mod acceptor;

pub use session::Session;

pub use socket::Socket;

pub use global::SocketType;

type EventLoop = mio::EventLoop<session_impl::SessionImpl>;

pub struct Message {
    pub header: Vec<u8>,
    pub body: Vec<u8>
}

impl Message {
	pub fn with_body(buffer: Vec<u8>) -> Message {
		Message {
			header: Vec::new(),
			body: buffer
		}
	}

	pub fn with_header_and_body(header: Vec<u8>, buffer: Vec<u8>) -> Message {
		Message { header: header, body: buffer }
	}

	pub fn len(&self) -> usize {
		self.header.len() + self.body.len()
	}

	pub fn get_header<'a>(&'a self) -> &'a [u8] {
		&self.header
	}

	pub fn get_body<'a>(&'a self) -> &'a [u8] {
		&self.body
	}

	pub fn to_buffer(self) -> Vec<u8> {
		self.body
	}

	pub fn explode(self) -> (Vec<u8>, Vec<u8>) {
		(self.header, self.body)
	}
}
