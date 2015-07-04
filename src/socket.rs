use event_loop_msg::EventLoopCmd as EventLoopCmd;
use event_loop_msg::SocketEvt as SocketEvt;

use mio;
use mio::NotifyError;

use std::sync::mpsc;
use std::io;

pub struct Socket {
	id: usize,
	cmd_sender: mio::Sender<EventLoopCmd>,
	evt_receiver: mpsc::Receiver<SocketEvt>
	// Could use https://github.com/polyfractal/bounded-spsc-queue ?
	// Only if there is one receiver per Socket and they are only 'Send'
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

	pub fn connect(&self, addr: &str) -> Result<(), io::Error> {
		debug!("Socket::connect {} -> {}", self.id, addr);
		let cmd = EventLoopCmd::ConnectSocket(self.id, addr.to_owned());

		// TODO : this should become a function or a macro ...
		if let Err(send_err) = self.cmd_sender.send(cmd) {
			let io_err = match send_err {
				NotifyError::Io(e) => e,
				NotifyError::Closed(_) => {
					io::Error::new(io::ErrorKind::Other, "cmd channel closed")
				},
				NotifyError::Full(_) => {
					io::Error::new(io::ErrorKind::WouldBlock, "cmd channel full")
				}
			};

			return Err(io_err);
		}

		match self.evt_receiver.recv() {
			Ok(SocketEvt::Connected) => Ok(()),
			Ok(SocketEvt::NotConnected(e)) => Err(e),
			Ok(_) => Err(io::Error::new(io::ErrorKind::Other, "unexpected evt")),
			Err(e) => Err(io::Error::new(io::ErrorKind::Other, "evt channel closed"))
		}
	}
}
