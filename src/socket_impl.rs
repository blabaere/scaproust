use std::sync::mpsc;

use event_loop_msg::EventLoopCmd as EventLoopCmd;
use event_loop_msg::SessionEvt as SessionEvt;
use event_loop_msg::SocketEvt as SocketEvt;

use protocol::Protocol as Protocol;

pub struct SocketImpl {
	protocol: Box<Protocol>,
	evt_sender: mpsc::Sender<SocketEvt>
	
}

impl SocketImpl {

	pub fn new(proto: Box<Protocol>, evt_tx: mpsc::Sender<SocketEvt>) -> SocketImpl {
		SocketImpl { protocol: proto, evt_sender: evt_tx }
	}

	pub fn pong(&self) {
		self.evt_sender.send(SocketEvt::Pong);
	}

	pub fn connect(&self, addr: &str) {
		self.evt_sender.send(SocketEvt::Pong);
		// get a transport (arg, fn ?)
		// connector = transport.connector()
		// self.add_connector(connector)
		// endpoint = connector.connect()
		// self.add_endpoint(endpoint)

		// connection retry should be done from here ?
		// since we are called from the event loop
		// it should return immediatly on success
		// or schedule a retry is case of failure
		// maybe this logic could be moved to a "SocketConnector" struct
		// which in turn would use a "TransportConnector" trait
	}

}
