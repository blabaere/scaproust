use std::collections::HashMap;

use std::sync::mpsc;

use std::io;

use mio;
use mio::{TryRead, TryWrite};

use event_loop_msg::EventLoopCmd as EventLoopCmd;
use event_loop_msg::EventLoopTimeout as EventLoopTimeout;
use event_loop_msg::SessionEvt as SessionEvt;
use event_loop_msg::SocketEvt as SocketEvt;

use protocol::Protocol as Protocol;
use transport::Transport as Transport;
use transport::Connection as Connection;
use transport;

use EventLoop;

pub struct SocketImpl {
	protocol: Box<Protocol>,
	evt_sender: mpsc::Sender<SocketEvt>,
	connections: HashMap<usize, Box<Connection>> 
}

impl SocketImpl {

	pub fn new(proto: Box<Protocol>, evt_tx: mpsc::Sender<SocketEvt>) -> SocketImpl {
		SocketImpl { 
			protocol: proto, 
			evt_sender: evt_tx,
			connections: HashMap::new()
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
		let connector = transport.connector(specific_addr, id);
		let connection = try!(connector.connect(event_loop));

		self.connections.insert(id, connection);
		self.evt_sender.send(SocketEvt::Connected);
		// get a transport (arg, fn ?)
		// socket_connector = self.connector()
		// tran_connector = transport.connector()
		// self.add_connector(connector) // sure ???
		// endpoint = connector.connect()
		// self.add_endpoint(endpoint)

		// connection retry should be done from here ?
		// since we are called from the event loop
		// it should return immediatly on success
		// or schedule a retry in case of failure
		// maybe this logic could be moved to a "SocketConnector" struct
		// which in turn would use a "TransportConnector" trait
		Ok(())
	}

	pub fn readable(&mut self, event_loop: &mut EventLoop, id: usize, hint: mio::ReadHint) {
		debug!("SocketImpl::readable {}", id);
		if let Some(connection) = self.connections.get_mut(&id) {
			connection.readable(event_loop, hint);
			let token = mio::Token(id); 
			let interest = mio::Interest::writable() | mio::Interest::hup() | mio::Interest::error();
			let poll_opt = mio::PollOpt::oneshot();
			event_loop.reregister(connection.as_evented(), token, interest, poll_opt);
		}
	}

	pub fn writable(&mut self, event_loop: &mut EventLoop, id: usize) {
		debug!("SocketImpl::writable {}", id);
		if let Some(connection) = self.connections.get_mut(&id) {
			match connection.writable(event_loop) {
				Ok(()) => {
					let token = mio::Token(id); 
					let interest = mio::Interest::readable() | mio::Interest::hup() | mio::Interest::error();
					let poll_opt = mio::PollOpt::oneshot();
					event_loop.reregister(connection.as_evented(), token, interest, poll_opt);
				},
				Err(_) => {
					debug!("SocketImpl::writable failed");
				}
			};

		}
		/*if let Some(connection) = self.connections.get_mut(&id) {
			let handshake = vec!(0, 83, 80, 0, 0, 80, 0, 0);

			match connection.try_write(&handshake) {
				Ok(_) => {

				},
				Err(e) => {
					connection.disconnect(event_loop, id);
					event_loop.timeout_ms(EventLoopTimeout::Reconnect(id, "tcp://127.0.0.1:5454".to_owned()), 500);
				}
			}
		}*/
	}

}
