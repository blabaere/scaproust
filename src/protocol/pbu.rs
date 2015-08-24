// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::sync::mpsc::Sender;
use std::collections::HashMap;
use std::io;
use std::boxed::FnBox;

use mio;

use super::Protocol;
use pipe::*;
use global::*;
use event_loop_msg::SocketEvt;
use EventLoop;
use Message;

pub struct Pub {
	pipes: HashMap<mio::Token, PubPipe>,
	evt_sender: Rc<Sender<SocketEvt>>,
	cancel_timeout: Option<Box<FnBox(&mut EventLoop)-> bool>>
}

impl Pub {
	pub fn new(evt_tx: Rc<Sender<SocketEvt>>) -> Pub {
		Pub { 
			pipes: HashMap::new(),
			evt_sender: evt_tx,
			cancel_timeout: None
		}
	}

	fn on_msg_send_ok(&mut self, event_loop: &mut EventLoop) {
		let _ = self.evt_sender.send(SocketEvt::MsgSent);

		self.on_msg_sending_finished();
		self.cancel_timeout.take().map(|cancel_timeout| cancel_timeout.call_box((event_loop,)));
	}

	fn on_msg_send_err(&mut self, event_loop: &mut EventLoop, err: io::Error) {
		let _ = self.evt_sender.send(SocketEvt::MsgNotSent(err));

		self.on_msg_sending_finished();
		self.cancel_timeout.take().map(|cancel_timeout| cancel_timeout.call_box((event_loop,)));
	}

	fn on_msg_sending_finished(&mut self) {
		for (_, pipe) in self.pipes.iter_mut() {
			pipe.on_msg_sending_finished();
		}
	}
}

impl Protocol for Pub {
	fn id(&self) -> u16 {
		SocketType::Pub.id()
	}

	fn peer_id(&self) -> u16 {
		SocketType::Sub.id()
	}

	fn add_pipe(&mut self, token: mio::Token, pipe: Pipe) {
		self.pipes.insert(token, PubPipe::new(pipe));
	}

	fn remove_pipe(&mut self, token: mio::Token) -> Option<Pipe> {
		self.pipes.remove(&token).map(|p| p.remove())
	}

	fn ready(&mut self, event_loop: &mut EventLoop, token: mio::Token, events: mio::EventSet) -> io::Result<()> {
		let mut result = Ok(());
		let mut msg_sent = false;

		if let Some(pipe) = self.pipes.get_mut(&token) {
			let mut sent = false;
			match pipe.ready(event_loop, events) {
				Ok(true)  => sent = true,
				Ok(false) => {},
				Err(e)    => result = Err(e)
			}

			if sent {
				msg_sent = true;
			} else {
				match pipe.resume_pending_send() {
					Ok(Some(true)) => msg_sent = true,
					Err(e)         => result = Err(e),
					_              => {}
				}
			}
		}

		let mut notify = false;
		if msg_sent | result.is_err() {
			let mut pipe_count = 0;
			let mut pipe_done_count = 0;

			for (_, pipe) in self.pipes.iter_mut() {
				match pipe.send_status() {
					Some(true) => {
						pipe_count += 1;
						pipe_done_count += 1;
					},
					Some(false) => {
						pipe_count += 1;
					}
					None => continue
				}

				notify = pipe_count > 0 && pipe_count == pipe_done_count;
			}

			if notify {
				for (_, pipe) in self.pipes.iter_mut() {
					pipe.reset_pending_send();
				}
				self.on_msg_send_ok(event_loop);
			}
		}

		result
	}

	fn send(&mut self, event_loop: &mut EventLoop, msg: Message, cancel_timeout: Box<FnBox(&mut EventLoop)-> bool>) {
		self.cancel_timeout = Some(cancel_timeout);

		let mut sent_count = 0;
		let mut sending_count = 0;
		let mut pending_count = 0;
		let shared_msg = Rc::new(msg);

		for (_, pipe) in self.pipes.iter_mut() {
			match pipe.send(shared_msg.clone()) {
				Ok(Some(true))  => sent_count    += 1,
				Ok(Some(false)) => sending_count += 1,
				Ok(None)        => pending_count += 1,
				Err(_)          => {}
				// this pipe looks dead, but it will be taken care of during next ready notification
			}
		}

		let sent = sent_count == self.pipes.len();
		let sending = sending_count > 0;
		let pending = pending_count > 0;

		if sent {
			self.on_msg_send_ok(event_loop);
		}

		if sent {
			for (_, pipe) in self.pipes.iter_mut() {
				pipe.reset_pending_send();
			}
		} else if pending == false && sending == false {
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

struct PubPipe {
    pipe: Pipe,
    pending_send: Option<Rc<Message>>,
    send_done: Option<bool> // if some, operation is finished or not ?
}

impl PubPipe {
	fn new(pipe: Pipe) -> PubPipe {
		PubPipe { 
			pipe: pipe,
			pending_send: None,
			send_done: None
		}
	}

	fn send_status(&self) -> Option<bool> {
		self.send_done.clone()
	}

	fn ready(&mut self, event_loop: &mut EventLoop, events: mio::EventSet) -> io::Result<bool> {
		let (sent, _) = try!(self.pipe.ready(event_loop, events));

		Ok(sent)
	}

	fn send(&mut self, msg: Rc<Message>) -> io::Result<Option<bool>> {
		let result = match self.pipe.send(msg) {
			Ok(SendStatus::Completed) => {
				self.pipe.cancel_sending();
				self.pending_send = None;
				self.send_done = Some(true);
				Ok(Some(true))
			},
			Ok(SendStatus::InProgress) => {
				self.pending_send = None;
				self.send_done = Some(false);
				Ok(Some(false))
			},
			Ok(SendStatus::Postponed(message)) => {
				self.pipe.cancel_sending();
				self.pending_send = Some(message);
				self.send_done = Some(false);
				Ok(None)
			}
			Err(e) => {
				self.pipe.cancel_sending();
				self.pending_send = None;
				self.send_done = Some(true);
				Err(e)
			}
		};

		result
	}

	fn on_send_timeout(&mut self) {
		self.pending_send = None;
		self.send_done = None;
		self.pipe.cancel_sending();
	}

	fn resume_pending_send(&mut self) -> io::Result<Option<bool>> {
		match self.pending_send.take() {
			None      => Ok(None),
			Some(msg) => self.send(msg)
		}
	}

	fn reset_pending_send(&mut self) {
		self.pending_send = None;
	}

	fn on_msg_sending_finished(&mut self) {
		self.pending_send = None;
		self.send_done = None;
		self.pipe.cancel_sending();
	}

	fn remove(self) -> Pipe {
		self.pipe
	}
}