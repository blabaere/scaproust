use std::sync::mpsc;

use event_loop_msg::EventLoopCmd as EventLoopCmd;
use event_loop_msg::SessionEvt as SessionEvt;
use event_loop_msg::SocketEvt as SocketEvt;

pub struct SocketImpl {
	evt_sender: mpsc::Sender<SocketEvt> 
}

impl SocketImpl {

	pub fn new(evt_tx: mpsc::Sender<SocketEvt>) -> SocketImpl {
		SocketImpl { evt_sender: evt_tx }
	}

	pub fn pong(&self) {
		self.evt_sender.send(SocketEvt::Pong);
	}

}
