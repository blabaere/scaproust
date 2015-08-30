// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::sync::mpsc;
use std::collections::HashMap;
use std::io;
use std::boxed::FnBox;

use mio;

use super::Protocol;
use pipe::*;
use endpoint::*;
use global::*;
use event_loop_msg::SocketEvt;
use EventLoop;
use Message;

pub struct Push {
	pipes: HashMap<mio::Token, Pipe>,
	evt_sender: Rc<mpsc::Sender<SocketEvt>>,
	cancel_timeout: Option<Box<FnBox(&mut EventLoop)-> bool>>
}

impl Push {
	pub fn new(evt_sender: Rc<mpsc::Sender<SocketEvt>>) -> Push {
		Push { 
			pipes: HashMap::new(),
			evt_sender: evt_sender,
			cancel_timeout: None
		}
	}

	fn on_msg_send_ok(&mut self, event_loop: &mut EventLoop) {
		let _ = self.evt_sender.send(SocketEvt::MsgSent);

		self.cancel_timeout.take().map(|cancel_timeout| cancel_timeout.call_box((event_loop,)));
	}

	fn on_msg_send_err(&mut self, event_loop: &mut EventLoop, err: io::Error) {
		let _ = self.evt_sender.send(SocketEvt::MsgNotSent(err));

		self.cancel_timeout.take().map(|cancel_timeout| cancel_timeout.call_box((event_loop,)));
	}
}

impl Protocol for Push {
	fn id(&self) -> u16 {
		SocketType::Push.id()
	}

	fn peer_id(&self) -> u16 {
		SocketType::Pull.id()
	}

	fn add_endpoint(&mut self, token: mio::Token, endpoint: Endpoint) {
		self.pipes.insert(token, Pipe::new(token, endpoint));
	}

	fn remove_endpoint(&mut self, token: mio::Token) -> Option<Endpoint> {
		self.pipes.remove(&token).map(|p| p.remove())
	}

	fn ready(&mut self, event_loop: &mut EventLoop, token: mio::Token, events: mio::EventSet) -> io::Result<()> {
		let mut msg_sending = false;
		let mut msg_sent = false;

		if let Some(pipe) = self.pipes.get_mut(&token) {
			let sent = try!(pipe.ready_tx(event_loop, events));

			if sent {
				msg_sent = true;
			} else {
				match try!(pipe.resume_pending_send()) {
					Some(true)  => msg_sent = true,
					Some(false) => msg_sending = true,
					None        => {}
				}
			}
		}

		if msg_sent {
			self.on_msg_send_ok(event_loop);
		}

		if msg_sending | msg_sent {
			for (_, pipe) in self.pipes.iter_mut() {
				pipe.reset_pending_send();
			}
		}

		Ok(())
	}

	fn send(&mut self, event_loop: &mut EventLoop, msg: Message, cancel_timeout: Box<FnBox(&mut EventLoop)-> bool>) {
		self.cancel_timeout = Some(cancel_timeout);

		let mut sent = false;
		let mut sending = false;
		let mut pending = false;
		let shared_msg = Rc::new(msg);

		for (_, pipe) in self.pipes.iter_mut() {
			match pipe.send(shared_msg.clone()) {
				Ok(Some(true))  => sent = true,
				Ok(Some(false)) => sending = true,
				Ok(None)        => pending = true,
				Err(_)          => continue 
				// this pipe looks dead, but it will be taken care of during next ready notification
			}

			if sent | sending {
				break;
			}
		}

		if sent {
			self.on_msg_send_ok(event_loop);
		}

		if sent | sending {
			for (_, pipe) in self.pipes.iter_mut() {
				pipe.reset_pending_send();
			}
		} else if pending == false {
			let err = io::Error::new(io::ErrorKind::NotConnected, "no connected endpoint");

			self.on_msg_send_err(event_loop, err);
		}
	}

	fn on_send_timeout(&mut self, event_loop: &mut EventLoop) {
		let err = io::Error::new(io::ErrorKind::TimedOut, "send timeout reached");

		self.on_msg_send_err(event_loop, err);

		for (_, pipe) in self.pipes.iter_mut() {
			pipe.on_send_timeout();
		}
	}

	fn recv(&mut self, _: &mut EventLoop, _: Box<FnBox(&mut EventLoop)-> bool>) {
		let err = other_io_error("recv not supported by protocol");
		let cmd = SocketEvt::MsgNotRecv(err);
		let _ = self.evt_sender.send(cmd);
	}
	
	fn on_recv_timeout(&mut self, _: &mut EventLoop) {
	}
}
