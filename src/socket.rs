use std::sync::mpsc;
use std::io;

use mio;
use mio::NotifyError;

use global::*;
use event_loop_msg::*;

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
	id: SocketId,
	cmd_sender: mio::Sender<EventLoopCmd>,
	evt_receiver: mpsc::Receiver<SocketEvt>
	// Could use https://github.com/polyfractal/bounded-spsc-queue ?
	// Maybe once a smart waiting strategy is available (like spin, then sleep 0, then sleep 1, then mutex ?)
}

impl Socket {
	pub fn new(
		id: SocketId,
		cmd_sender: mio::Sender<EventLoopCmd>,
		evt_receiver: mpsc::Receiver<SocketEvt>) -> Socket {

		Socket {
			id: id,
			cmd_sender: cmd_sender,
			evt_receiver: evt_receiver
		}

	}

	fn send_cmd(&self, socket_cmd: SocketCmd) -> Result<(), io::Error> {
		let cmd = EventLoopCmd::SocketLevel(self.id, socket_cmd);

		self.cmd_sender.send(cmd).map_err(|e| convert_notify_err(e))
	}
	
	pub fn ping(&self) -> Result<(), io::Error> {
		try!(self.send_cmd(SocketCmd::Ping));

		match self.evt_receiver.recv() {
			Ok(SocketEvt::Pong) => Ok(()),
			Ok(_) => Err(io::Error::new(io::ErrorKind::Other, "unexpected evt")),
			Err(_) => Err(io::Error::new(io::ErrorKind::Other, "evt channel closed"))
		}
	}

	// TODO: return an Endpoint struct with a shutdown method instead of '()'
	pub fn connect(&self, addr: &str) -> Result<(), io::Error> {
		let cmd = SocketCmd::Connect(addr.to_owned());
		
		try!(self.send_cmd(cmd));

		match self.evt_receiver.recv() {
			Ok(SocketEvt::Connected) => Ok(()),
			Ok(SocketEvt::NotConnected(e)) => Err(e),
			Ok(_) => Err(io::Error::new(io::ErrorKind::Other, "unexpected evt")),
			Err(_) => Err(io::Error::new(io::ErrorKind::Other, "evt channel closed"))
		}
	}

	// TODO: return an Endpoint struct with a shutdown method instead of '()'
	pub fn bind(&self, addr: &str) -> Result<(), io::Error> {
		let cmd = SocketCmd::Bind(addr.to_owned());
		
		try!(self.send_cmd(cmd));

		match self.evt_receiver.recv() {
			Ok(SocketEvt::Bound) => Ok(()),
			Ok(SocketEvt::NotBound(e)) => Err(e),
			Ok(_) => Err(io::Error::new(io::ErrorKind::Other, "unexpected evt")),
			Err(_) => Err(io::Error::new(io::ErrorKind::Other, "evt channel closed"))
		}
	}

	pub fn send(&self, buffer: Vec<u8>) -> Result<(), io::Error> {
		self.send_msg(Message::new(buffer))
	}

	pub fn send_msg(&self, msg: Message) -> Result<(), io::Error> {
		let cmd = SocketCmd::SendMsg(msg);

		try!(self.send_cmd(cmd));

		match self.evt_receiver.recv() {
			Ok(SocketEvt::MsgSent) => Ok(()),
			Ok(SocketEvt::MsgNotSent(e)) => Err(e),
			Ok(_) => Err(io::Error::new(io::ErrorKind::Other, "unexpected evt")),
			Err(_) => Err(io::Error::new(io::ErrorKind::Other, "evt channel closed"))
		}
	}

	pub fn recv_msg(&self) -> Result<Message, io::Error> {
		try!(self.send_cmd(SocketCmd::RecvMsg));

		match self.evt_receiver.recv() {
			Ok(SocketEvt::MsgRecv(msg)) => Ok(msg),
			Ok(SocketEvt::MsgNotRecv(e)) => Err(e),
			Ok(_) => Err(io::Error::new(io::ErrorKind::Other, "unexpected evt")),
			Err(_) => Err(io::Error::new(io::ErrorKind::Other, "evt channel closed"))
		}
	}
}