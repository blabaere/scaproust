use std::sync::mpsc::Receiver;
use std::io;
use std::time;

use mio::Sender;

use global::*;
use event_loop_msg::*;

use Message;

pub struct Socket {
	id: SocketId,
	cmd_sender: Sender<EventLoopCmd>,
	evt_receiver: Receiver<SocketEvt>
	// Could use https://github.com/polyfractal/bounded-spsc-queue ?
	// Maybe once a smart waiting strategy is available (like spin, then sleep 0, then sleep 1, then mutex ?)
}

impl Socket {
	pub fn new(id: SocketId, cmd_tx: Sender<EventLoopCmd>, evt_rx: Receiver<SocketEvt>) -> Socket {
		Socket { id: id, cmd_sender: cmd_tx, evt_receiver: evt_rx }
	}

	fn send_cmd(&self, socket_cmd: SocketCmd) -> Result<(), io::Error> {
		let cmd = EventLoopCmd::SocketLevel(self.id, socket_cmd);

		self.cmd_sender.send(cmd).map_err(|e| convert_notify_err(e))
	}

	pub fn connect(&mut self, addr: &str) -> Result<(), io::Error> {
		let cmd = SocketCmd::Connect(addr.to_owned());
		
		try!(self.send_cmd(cmd));

		match self.evt_receiver.recv() {
			Ok(SocketEvt::Connected)       => Ok(()),
			Ok(SocketEvt::NotConnected(e)) => Err(e),
			Ok(_)                          => Err(other_io_error("unexpected evt")),
			Err(_)                         => Err(other_io_error("evt channel closed"))
		}
	}

	pub fn bind(&mut self, addr: &str) -> Result<(), io::Error> {
		let cmd = SocketCmd::Bind(addr.to_owned());
		
		try!(self.send_cmd(cmd));

		match self.evt_receiver.recv() {
			Ok(SocketEvt::Bound)       => Ok(()),
			Ok(SocketEvt::NotBound(e)) => Err(e),
			Ok(_)                      => Err(other_io_error("unexpected evt")),
			Err(_)                     => Err(other_io_error("evt channel closed"))
		}
	}

	pub fn send(&mut self, buffer: Vec<u8>) -> Result<(), io::Error> {
		self.send_msg(Message::with_body(buffer))
	}

	pub fn send_msg(&mut self, msg: Message) -> Result<(), io::Error> {
		let cmd = SocketCmd::SendMsg(msg);

		try!(self.send_cmd(cmd));

		match self.evt_receiver.recv() {
			Ok(SocketEvt::MsgSent)       => Ok(()),
			Ok(SocketEvt::MsgNotSent(e)) => Err(e),
			Ok(_)                        => Err(other_io_error("unexpected evt")),
			Err(_)                       => Err(other_io_error("evt channel closed"))
		}
	}

	pub fn recv(&mut self) -> Result<Vec<u8>, io::Error> {
		self.recv_msg().map(|msg| msg.to_buffer())
	}

	pub fn recv_msg(&mut self) -> Result<Message, io::Error> {
		try!(self.send_cmd(SocketCmd::RecvMsg));

		match self.evt_receiver.recv() {
			Ok(SocketEvt::MsgRecv(msg))  => Ok(msg),
			Ok(SocketEvt::MsgNotRecv(e)) => Err(e),
			Ok(_)                        => Err(other_io_error("unexpected evt")),
			Err(_)                       => Err(other_io_error("evt channel closed"))
		}
	}

	fn set_option(&mut self, option: SocketOption) -> io::Result<()> {
		let cmd = SocketCmd::SetOption(option);

		try!(self.send_cmd(cmd));

		match self.evt_receiver.recv() {
			Ok(SocketEvt::OptionSet)       => Ok(()),
			Ok(SocketEvt::OptionNotSet(e)) => Err(e),
			Ok(_)                          => Err(other_io_error("unexpected evt")),
			Err(_)                         => Err(other_io_error("evt channel closed"))
		}
	}

	pub fn set_send_timeout(&mut self, timeout: time::Duration) -> io::Result<()> {
		self.set_option(SocketOption::SendTimeout(timeout))
	}

	pub fn set_recv_timeout(&mut self, timeout: time::Duration) -> io::Result<()> {
		self.set_option(SocketOption::RecvTimeout(timeout))
	}
}
