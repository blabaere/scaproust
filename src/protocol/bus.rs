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
use endpoint::*;
use global::*;
use event_loop_msg::SocketEvt;
use EventLoop;
use Message;

pub struct Bus {
	pipes: HashMap<mio::Token, Pipe>,
	evt_sender: Rc<Sender<SocketEvt>>,
	cancel_send_timeout: Option<Box<FnBox(&mut EventLoop)-> bool>>,
	cancel_recv_timeout: Option<Box<FnBox(&mut EventLoop)-> bool>>
}

impl Bus {
	pub fn new(evt_tx: Rc<Sender<SocketEvt>>) -> Bus {
		Bus { 
			pipes: HashMap::new(),
			evt_sender: evt_tx,
			cancel_send_timeout: None,
			cancel_recv_timeout: None
		}
	}

	fn on_msg_send_ok(&mut self, event_loop: &mut EventLoop) {
		let evt = SocketEvt::MsgSent;
		let timeout = self.cancel_send_timeout.take();

		self.on_msg_sending_finished();
		self.send_event_and_cancel_timeout(event_loop, evt, timeout)
	}

	fn on_msg_send_err(&mut self, event_loop: &mut EventLoop, err: io::Error) {
		let evt = SocketEvt::MsgNotSent(err);
		let timeout = self.cancel_send_timeout.take();

		self.on_msg_sending_finished();
		self.send_event_and_cancel_timeout(event_loop, evt, timeout)
	}

	fn on_msg_sending_finished(&mut self) {
		for (_, pipe) in self.pipes.iter_mut() {
			pipe.on_msg_sending_finished();
		}
	}

	fn on_msg_recv_ok(&mut self, event_loop: &mut EventLoop, msg: Message) {
		let evt = SocketEvt::MsgRecv(msg);
		let timeout = self.cancel_recv_timeout.take();

		self.send_event_and_cancel_timeout(event_loop, evt, timeout)
	}

	fn on_msg_recv_err(&mut self, event_loop: &mut EventLoop, err: io::Error) {
		let evt = SocketEvt::MsgNotRecv(err);
		let timeout = self.cancel_recv_timeout.take();

		self.send_event_and_cancel_timeout(event_loop, evt, timeout)
	}

	fn send_event_and_cancel_timeout(&self, 
		event_loop: &mut EventLoop, 
		evt: SocketEvt, 
		timeout: Option<Box<FnBox(&mut EventLoop)-> bool>>) {
		
		let _ = self.evt_sender.send(evt);

		timeout.map(|cancel_timeout| cancel_timeout.call_box((event_loop,)));
	}
}

impl Protocol for Bus {
	fn id(&self) -> u16 {
		SocketType::Bus.id()
	}

	fn peer_id(&self) -> u16 {
		SocketType::Bus.id()
	}

	fn add_endpoint(&mut self, token: mio::Token, endpoint: Endpoint) {
		self.pipes.insert(token, Pipe::new(token, endpoint));
	}

	fn remove_endpoint(&mut self, token: mio::Token) -> Option<Endpoint> {
		self.pipes.remove(&token).map(|p| p.remove())
	}

	fn ready(&mut self, event_loop: &mut EventLoop, token: mio::Token, events: mio::EventSet) -> io::Result<()> {
		let mut result = Ok(());
		let mut sent_msg = false;
		let mut send_msg_error = false;
		let mut received_msg = None;
		let mut receiving_msg = false;

		if let Some(pipe) = self.pipes.get_mut(&token) {
			match pipe.ready(event_loop, events) {
				Ok((sent, received))  => {

					if sent {
						sent_msg = true;
					} else {
						match pipe.resume_pending_send() {
							Ok(Some(true)) => sent_msg = true,
							Err(e)         =>  {
								send_msg_error = true;
								result = Err(e);
							},
							_              => {}
						}
					}

					if received.is_some() {
						received_msg = received;
					} else {
						match pipe.resume_pending_recv() {
							Ok(Some(RecvStatus::Completed(msg))) => received_msg = Some(msg),
							Ok(Some(RecvStatus::InProgress))     => receiving_msg = true,
							Ok(Some(RecvStatus::Postponed))      => {},
							Ok(None)                             => {},
							Err(e)                               => result = Err(e)
						}
					}
				},
				Err(e)    => {
					send_msg_error = true;
					result = Err(e);
				}
			}
		}

		let mut notify_sent = false;
		if sent_msg | send_msg_error {
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

				notify_sent = pipe_count > 0 && pipe_count == pipe_done_count;
			}

			if notify_sent {
				for (_, pipe) in self.pipes.iter_mut() {
					pipe.reset_pending_send();
				}
				self.on_msg_send_ok(event_loop);
			}
		}

		if received_msg.is_some() | receiving_msg {
			for (_, pipe) in self.pipes.iter_mut() {
				pipe.reset_pending_recv();
			}
		}

		if received_msg.is_some() {
			let msg = received_msg.unwrap();

			self.on_msg_recv_ok(event_loop, msg);
		}

		result
	}

	fn send(&mut self, event_loop: &mut EventLoop, msg: Message, cancel_timeout: Box<FnBox(&mut EventLoop)-> bool>) {
		self.cancel_send_timeout = Some(cancel_timeout);

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

	fn recv(&mut self, event_loop: &mut EventLoop, cancel_timeout: Box<FnBox(&mut EventLoop)-> bool>) {
		self.cancel_recv_timeout = Some(cancel_timeout);

		let mut received = None;
		let mut receiving = false;
		let mut pending = false;

		for (_, pipe) in self.pipes.iter_mut() {
			match pipe.recv() {
				Ok(RecvStatus::Completed(msg)) => received = Some(msg),
				Ok(RecvStatus::InProgress)     => receiving = true,
				Ok(RecvStatus::Postponed)      => pending = true,
				Err(_)                         => continue
			}

			if received.is_some() | receiving {
				break;
			}
		}

		if received.is_some() | receiving {
			for (_, pipe) in self.pipes.iter_mut() {
				pipe.reset_pending_recv();
			}
		} else if pending == false {
			let err = io::Error::new(io::ErrorKind::NotConnected, "no connected endpoint");

			self.on_msg_recv_err(event_loop, err);
		}

		if received.is_some() {
			let msg = received.unwrap();

			self.on_msg_recv_ok(event_loop, msg);
		}
	}
	
	fn on_recv_timeout(&mut self, event_loop: &mut EventLoop) {
		let err = io::Error::new(io::ErrorKind::TimedOut, "recv timeout reached");

		self.on_msg_recv_err(event_loop, err);

		for (_, pipe) in self.pipes.iter_mut() {
			pipe.on_recv_timeout();
		}
	}
}
