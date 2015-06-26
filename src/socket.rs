use event_loop_msg::EventLoopCmd as EventLoopCmd;
use event_loop_msg::SocketEvt as SocketEvt;

use mio;

use std::sync::mpsc;

pub struct Socket {
	id: usize,
	cmd_sender: mio::Sender<EventLoopCmd>,
	evt_receiver: mpsc::Receiver<SocketEvt>
	// Could use https://github.com/polyfractal/bounded-spsc-queue ?
	// Only if there is one receiver per NanoSocket and they are only 'Send'
}

impl Socket {
	pub fn new(
		id: usize,
		cmd_sender: mio::Sender<EventLoopCmd>,
		evt_receiver: mpsc::Receiver<SocketEvt>) -> Socket {

		Socket {
			id: id,
			cmd_sender: cmd_sender,
			evt_receiver: evt_receiver
		}
		
	}
	pub fn ping(&self) {
		let cmd = EventLoopCmd::PingSocket(self.id);

		self.cmd_sender.send(cmd);
		self.evt_receiver.recv().unwrap();
	}
}
