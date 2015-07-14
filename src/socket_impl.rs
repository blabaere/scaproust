use std::rc::Rc;
use std::ops::Deref;
use std::sync::mpsc;
use std::io;

use mio;

use event_loop_msg::EventLoopCmd as EventLoopCmd;
use event_loop_msg::EventLoopTimeout as EventLoopTimeout;
use event_loop_msg::SocketEvt as SocketEvt;

use protocol::Protocol as Protocol;
use pipe::Pipe as Pipe;
use transport::{create_transport, Connection};

use EventLoop;
use Message;

pub struct SocketImpl {
	id: usize,
	protocol: Box<Protocol>,
	evt_sender: mpsc::Sender<SocketEvt>,
}

impl SocketImpl {

	pub fn new(id: usize, proto: Box<Protocol>, evt_tx: mpsc::Sender<SocketEvt>) -> SocketImpl {
		SocketImpl { 
			id: id,
			protocol: proto, 
			evt_sender: evt_tx
		}
	}

	pub fn pong(&self) {
		self.evt_sender.send(SocketEvt::Pong);
	}

	pub fn connect(&mut self, addr: String, event_loop: &mut EventLoop, id: usize) {
		debug!("[{}] pipe [{}] connect: '{}'", self.id, id, addr);

		match self.create_connection(&addr) {
			Ok(connection) => {
				let mut pipe = Pipe::new(addr.clone(), id, &*self.protocol, connection);

				pipe.init(event_loop);

				self.protocol.add_pipe(id, pipe);
				self.evt_sender.send(SocketEvt::Connected);
			}
			Err(e) => {
				self.evt_sender.send(SocketEvt::NotConnected(e));
			}
		}
	}

	pub fn reconnect(&mut self, addr: String, event_loop: &mut EventLoop, id: usize) {
		debug!("[{}] pipe [{}] reconnect: '{}'", self.id, id, addr);

		match self.create_connection(&addr) {
			Ok(connection) => {
				let mut pipe = Pipe::new(addr.clone(), id, &*self.protocol, connection);

				pipe.init(event_loop);

				self.protocol.add_pipe(id, pipe);
			}
			Err(e) => {
				// ??? reschedule ?
			}
		}
	}

	fn create_connection(&self, addr: &str) -> Result<Box<Connection>, io::Error> {

		let addr_parts: Vec<&str> = addr.split("://").collect();
		let scheme = addr_parts[0];
		let specific_addr = addr_parts[1];
		let transport = create_transport(scheme);
		
		transport.connect(specific_addr)
	}

	pub fn send(&mut self, event_loop: &mut EventLoop, msg: Message) {
		debug!("[{}] send", self.id);
		self.protocol.send(event_loop, msg);
		self.evt_sender.send(SocketEvt::MsgSent); // pretend it went fine
	}

	pub fn ready(&mut self, event_loop: &mut EventLoop, id: usize, events: mio::EventSet) {
		debug!("[{}] pipe [{}] ready: '{:?}'", self.id, id, events);
		self.protocol.
			ready(event_loop, id, events).
			unwrap_or_else(|e| self.on_pipe_error(event_loop, id, e));
	}

	fn on_pipe_error(&mut self, event_loop: &mut EventLoop, id: usize, err: io::Error) {
		debug!("[{}] pipe [{}] error: '{:?}'", self.id, id, err);

		match self.protocol.remove_pipe(id) {
			Some(addr) => {
				event_loop.timeout_ms(EventLoopTimeout::Reconnect(id, addr), 200);
			},
			_ => {}
		}
	}
}
