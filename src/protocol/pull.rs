use mio;

use super::Protocol;
use std::io;

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

	fn send(&mut self, event_loop: &mut EventLoop, msg: Message) {

	}
}