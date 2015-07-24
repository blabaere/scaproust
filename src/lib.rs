#![crate_name = "scaproust"]

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

pub use session::Session;

pub use socket::Socket;

pub use global::SocketType;

type EventLoop = mio::EventLoop<session_impl::SessionImpl>;

pub struct Message {
	// one day these vec should come from a pool
    header: Vec<u8>,
    body: Vec<u8>
}

impl Message {
	pub fn new(buffer: Vec<u8>) -> Message {
		Message {
			header: Vec::new(),
			body: buffer
		}
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
}
