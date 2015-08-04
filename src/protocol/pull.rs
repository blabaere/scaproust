use std::io;
use std::boxed::FnBox;

use mio;

use super::Protocol;

use pipe::Pipe as Pipe;
use global::SocketType as SocketType;
use EventLoop;
use Message;

#[derive(Debug, Copy, Clone)]
pub struct Pull;

impl Protocol for Pull {
	fn id(&self) -> u16 {
		SocketType::Pull.id()
	}

	fn peer_id(&self) -> u16 {
		SocketType::Push.id()
	}

	fn add_pipe(&mut self, token: mio::Token, _: Pipe) {
	}

	fn remove_pipe(&mut self, token: mio::Token) -> Option<Pipe> {
		None
	}

	fn ready(&mut self, event_loop: &mut EventLoop, token: mio::Token, events: mio::EventSet) -> io::Result<()> {
		Ok(())
	}

	fn send(&mut self, event_loop: &mut EventLoop, msg: Message, cancel_timeout: Box<FnBox(&mut EventLoop)-> bool>) {

	}

	fn on_send_timeout(&mut self, event_loop: &mut EventLoop) {

	}
}