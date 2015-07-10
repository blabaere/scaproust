use std::sync::mpsc;
use std::io;

use mio;
use mio::NotifyError;

use event_loop_msg::EventLoopCmd as EventLoopCmd;
use event_loop_msg::SocketEvt as SocketEvt;

use Message;

fn convert_notify_err<T>(err: NotifyError<T>) -> io::Error {
	match err {
		NotifyError::Io(e) => e,
		NotifyError::Closed(_) => {
			io::Error::new(io::ErrorKind::Other, "cmd channel closed")
		},
		NotifyError::Full(_) => {
			io::Error::new(io::ErrorKind::WouldBlock, "cmd channel full")
		}
	}
}

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

	fn send_cmd(&self, cmd: EventLoopCmd) -> Result<(), io::Error> {
		self.cmd_sender.send(cmd).map_err(|e| convert_notify_err(e))
	}
	
	pub fn ping(&self) -> Result<(), io::Error> {
		let cmd = EventLoopCmd::PingSocket(self.id);

		try!(self.send_cmd(cmd));

		match self.evt_receiver.recv() {
			Ok(SocketEvt::Pong) => Ok(()),
			Ok(_) => Err(io::Error::new(io::ErrorKind::Other, "unexpected evt")),
			Err(_) => Err(io::Error::new(io::ErrorKind::Other, "evt channel closed"))
		}
	}

	// TODO: return an Endpoint struct with a shutdown method instead of '()'
	pub fn connect(&self, addr: &str) -> Result<(), io::Error> {
		debug!("Socket::connect {} -> {}", self.id, addr);
		let cmd = EventLoopCmd::ConnectSocket(self.id, addr.to_owned());

		try!(self.send_cmd(cmd));

		match self.evt_receiver.recv() {
			Ok(SocketEvt::Connected) => Ok(()),
			Ok(SocketEvt::NotConnected(e)) => Err(e),
			Ok(_) => Err(io::Error::new(io::ErrorKind::Other, "unexpected evt")),
			Err(_) => Err(io::Error::new(io::ErrorKind::Other, "evt channel closed"))
		}
	}

	pub fn send(&self, buffer: Vec<u8>) -> Result<(), io::Error> {
		let msg = Message::new(buffer);
		let cmd = EventLoopCmd::SendMsg(self.id, msg);

		try!(self.send_cmd(cmd));

		match self.evt_receiver.recv() {
			Ok(SocketEvt::MsgSent) => Ok(()),
			Ok(SocketEvt::MsgNotSent(e)) => Err(e),
			Ok(_) => Err(io::Error::new(io::ErrorKind::Other, "unexpected evt")),
			Err(_) => Err(io::Error::new(io::ErrorKind::Other, "evt channel closed"))
		}
	}
}
