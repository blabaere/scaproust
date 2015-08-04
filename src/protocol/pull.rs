use std::rc::Rc;
use std::sync::mpsc;
use std::collections::HashMap;
use std::io;
use std::boxed::FnBox;

use mio;

use super::Protocol;
use pipe::*;
use global::*;
use event_loop_msg::SocketEvt as SocketEvt;
use EventLoop;
use Message;

pub struct Pull {
	pipes: HashMap<mio::Token, Pipe>,
	evt_sender: Rc<mpsc::Sender<SocketEvt>>,
	cancel_timeout: Option<Box<FnBox(&mut EventLoop)-> bool>>
}

impl Pull {
	pub fn new(evt_sender: Rc<mpsc::Sender<SocketEvt>>) -> Pull {
		Pull { 
			pipes: HashMap::new(),
			evt_sender: evt_sender,
			cancel_timeout: None
		}
	}
}

impl Protocol for Pull {
	fn id(&self) -> u16 {
		SocketType::Pull.id()
	}

	fn peer_id(&self) -> u16 {
		SocketType::Push.id()
	}

	fn add_pipe(&mut self, token: mio::Token, pipe: Pipe) {
		self.pipes.insert(token, pipe);
	}

	fn remove_pipe(&mut self, token: mio::Token) -> Option<Pipe> {
		self.pipes.remove(&token)
	}

	fn ready(&mut self, event_loop: &mut EventLoop, token: mio::Token, events: mio::EventSet) -> io::Result<()> {
		if let Some(pipe) = self.pipes.get_mut(&token) {
			let (_, received) = try!(pipe.ready(event_loop, events));

			if received {
				let msg = Message::new(Vec::new());
				let _ = self.evt_sender.send(SocketEvt::MsgRecv(msg));
			}
		}

		Ok(())
	}

	fn send(&mut self, _: &mut EventLoop, _: Message, _: Box<FnBox(&mut EventLoop)-> bool>) {
		let err = other_io_error("send not supported by protocol");
		let cmd = SocketEvt::MsgNotSent(err);
		let _ = self.evt_sender.send(cmd);
	}

	fn on_send_timeout(&mut self, _: &mut EventLoop) {

	}

	fn recv(&mut self, event_loop: &mut EventLoop, cancel_timeout: Box<FnBox(&mut EventLoop)-> bool>) {
		self.cancel_timeout = Some(cancel_timeout);

		for (_, pipe) in self.pipes.iter_mut() {
			match pipe.recv() {
				Ok(RecvStatus::Postponed)      => debug!("recv postponed !"),
				Ok(RecvStatus::InProgress)     => debug!("recv in progress ..."),
				Ok(RecvStatus::Completed(msg)) => {
					debug!("recv completed !");
					let _ = self.evt_sender.send(SocketEvt::MsgRecv(msg));
				},
				Err(e) => {
					debug!("recv failed: '{:?}' !", e);
					let _ = self.evt_sender.send(SocketEvt::MsgNotRecv(e));
				}
			}
		}
	}

	fn on_recv_timeout(&mut self, event_loop: &mut EventLoop) {

	}
}
