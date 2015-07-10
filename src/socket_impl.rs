use std::rc::Rc;
use std::collections::HashMap;

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
	protocol: Rc<Box<Protocol>>,
	evt_sender: mpsc::Sender<SocketEvt>,
	pipes: HashMap<usize, Pipe> 
}

impl SocketImpl {

	pub fn new(proto: Box<Protocol>, evt_tx: mpsc::Sender<SocketEvt>) -> SocketImpl {
		SocketImpl { 
			protocol: Rc::new(proto), 
			evt_sender: evt_tx,
			pipes: HashMap::new()
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

		// TODO : pipe does not need the whole protocol, just the ids
		// but the protocol will probably need to know about all the pipes
		// for example Push will check all the conn to find the first writable one
		let mut pipe = Pipe::new(id, self.protocol.clone(), connection);

		pipe.init(event_loop);

		self.pipes.insert(id, pipe);
		self.evt_sender.send(SocketEvt::Connected);

		Ok(())
	}

	pub fn send(&mut self, msg: Message) {
	}

	pub fn ready(&mut self, event_loop: &mut EventLoop, id: usize, events: mio::EventSet) {
		if let Some(pipe) = self.pipes.get_mut(&id) {
			pipe.ready(event_loop, events);
		}
	}

}
