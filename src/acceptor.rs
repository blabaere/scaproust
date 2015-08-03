use std::io;

use mio;

use global;
use transport::{ Listener, Connection };
use EventLoop;

pub struct Acceptor {
	token: mio::Token,
	addr: String, 
	listener: Box<Listener>
}

impl Acceptor {
	pub fn new(token: mio::Token, addr: String, listener: Box<Listener>) -> Acceptor {
		Acceptor { 
			token: token,
			addr: addr,
			listener: listener 
		}
	}

	pub fn init(&mut self, event_loop: &mut EventLoop) -> io::Result<()> {
		let io = self.listener.as_evented();
		let interest = mio::EventSet::error() | mio::EventSet::readable();

		event_loop.register_opt(io, self.token, interest, mio::PollOpt::edge())
	}

	pub fn ready(&mut self, _: &mut EventLoop, events: mio::EventSet) -> io::Result<Vec<Box<Connection>>> {
		if events.is_readable() {
			self.listener.accept()
		} else {
			Err(global::other_io_error("tcp listener ready but not readable"))
		}
	}

	pub fn addr(self) -> String {
		self.addr
	}
}