use std::rc::Rc;
use std::sync::mpsc;
use std::io;

use mio;

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
	evt_sender: Rc<mpsc::Sender<SocketEvt>>
}

impl SocketImpl {

	pub fn new(id: usize, proto: Box<Protocol>, evt_tx: Rc<mpsc::Sender<SocketEvt>>) -> SocketImpl {
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

		let evt = match self.create_connection(&addr).and_then(|c| self.on_connected(addr, event_loop, id, c)) {
			Ok(_) => SocketEvt::Connected,
			Err(e) => SocketEvt::NotConnected(e)
		};

		self.evt_sender.send(evt);
	}

	pub fn reconnect(&mut self, addr: String, event_loop: &mut EventLoop, id: usize) {
		debug!("[{}] pipe [{}] reconnect: '{}'", self.id, id, addr);

		self.create_connection(&addr).
			and_then(|c| self.on_connected(addr, event_loop, id, c)).
			unwrap_or_else(|e| self.on_pipe_error(event_loop, id, e));
	}

	fn create_connection(&self, addr: &str) -> io::Result<Box<Connection>> {

		let addr_parts: Vec<&str> = addr.split("://").collect();
		let scheme = addr_parts[0];
		let specific_addr = addr_parts[1];
		let transport = create_transport(scheme);
		
		transport.connect(specific_addr)
	}

	fn on_connected(&mut self, addr: String, event_loop: &mut EventLoop, id: usize, conn: Box<Connection>) -> io::Result<()> {
		let evt_sender = self.evt_sender.clone();
		let mut pipe = Pipe::new(id, addr, &*self.protocol, conn, evt_sender);

		pipe.init(event_loop).and_then(|_| Ok(self.protocol.add_pipe(id, pipe)))
	}

	pub fn send(&mut self, event_loop: &mut EventLoop, msg: Message) {
		debug!("[{}] send", self.id);
		self.protocol.send(event_loop, msg);
	}

	pub fn ready(&mut self, event_loop: &mut EventLoop, id: usize, events: mio::EventSet) {
		debug!("[{}] pipe [{}] ready: '{:?}'", self.id, id, events);
		self.protocol.
			ready(event_loop, id, events).
			unwrap_or_else(|e| self.on_pipe_error(event_loop, id, e));
	}

	fn on_pipe_error(&mut self, event_loop: &mut EventLoop, id: usize, err: io::Error) {
		debug!("[{}] pipe [{}] error: '{:?}'", self.id, id, err);

		if let Some(addr) = self.protocol.remove_pipe(id) {
			event_loop.timeout_ms(EventLoopTimeout::Reconnect(id, addr), 200);
		}
	}
}
