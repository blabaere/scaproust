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
use transport;

use EventLoop;
use Message;

pub struct SocketImpl {
	protocol: Box<Protocol>,
	evt_sender: mpsc::Sender<SocketEvt>,
}

impl SocketImpl {

	pub fn new(proto: Box<Protocol>, evt_tx: mpsc::Sender<SocketEvt>) -> SocketImpl {
		SocketImpl { 
			protocol: proto, 
			evt_sender: evt_tx
		}
	}

	pub fn pong(&self) {
		self.evt_sender.send(SocketEvt::Pong);
	}

	pub fn connect(&mut self, addr: &str, event_loop: &mut EventLoop, id: usize) -> Result<(), io::Error> {
		debug!("SocketImpl::connect {} -> {}", id, addr);

		let addr_parts: Vec<&str> = addr.split("://").collect();
		let scheme = addr_parts[0];
		let specific_addr = addr_parts[1];
		let transport = transport::create_transport(scheme);
		let connection = transport.connect(specific_addr).unwrap();
		let mut pipe = Pipe::new(id, &*self.protocol, connection);

		pipe.init(event_loop);

		self.protocol.add_pipe(id, pipe);

		self.evt_sender.send(SocketEvt::Connected);

		Ok(())
	}

	pub fn send(&mut self, event_loop: &mut EventLoop, msg: Message) {
		self.protocol.send(event_loop, msg);
		self.evt_sender.send(SocketEvt::MsgSent); // pretend it went fine
	}

	pub fn ready(&mut self, event_loop: &mut EventLoop, id: usize, events: mio::EventSet) {
		self.protocol.ready(event_loop, id, events);
	}

}
