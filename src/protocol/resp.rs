// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::sync::mpsc::Sender;
use std::io;
use std::boxed::FnBox;

use mio;

use byteorder::{ BigEndian, WriteBytesExt, ReadBytesExt };

use super::Protocol;
use pipe::*;
use global::*;
use event_loop_msg::SocketEvt;
use EventLoop;
use Message;

pub struct Resp {
	pipe: Option<RespPipe>,
	evt_sender: Rc<Sender<SocketEvt>>,
	cancel_send_timeout: Option<Box<FnBox(&mut EventLoop)-> bool>>,
	cancel_recv_timeout: Option<Box<FnBox(&mut EventLoop)-> bool>>,
	backtrace: Vec<u8>,
	ttl: u8
}

impl Resp {
	pub fn new(evt_sender: Rc<Sender<SocketEvt>>) -> Resp {
		Resp { 
			pipe: None,
			evt_sender: evt_sender,
			cancel_send_timeout: None,
			cancel_recv_timeout: None,
			backtrace: Vec::with_capacity(64),
			ttl: 8
		}
	}

	fn on_msg_send_ok(&mut self, event_loop: &mut EventLoop) {
		let evt = SocketEvt::MsgSent;
		let timeout = self.cancel_send_timeout.take();

		self.send_event_and_cancel_timeout(event_loop, evt, timeout)
	}

	fn on_msg_send_err(&mut self, event_loop: &mut EventLoop, err: io::Error) {
		let evt = SocketEvt::MsgNotSent(err);
		let timeout = self.cancel_send_timeout.take();

		self.send_event_and_cancel_timeout(event_loop, evt, timeout)
	}

	fn on_msg_recv_ok(&mut self, event_loop: &mut EventLoop, msg: Message) {
		self.save_received_header_to_backtrace(&msg);

		let evt = SocketEvt::MsgRecv(msg);
		let timeout = self.cancel_recv_timeout.take();

		self.send_event_and_cancel_timeout(event_loop, evt, timeout)
	}

	fn save_received_header_to_backtrace(&mut self, msg: &Message) {
		self.backtrace.clear();
		self.backtrace.push_all(msg.get_header());
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

	fn raw_msg_to_msg(&mut self, pipe_token: mio::Token, raw_msg: Message) -> io::Result<Option<Message>> {
		let (mut header, mut body) = raw_msg.explode();
		let pipe_id = pipe_token.as_usize() as u32;

		header.reserve(4);
		try!(header.write_u32::<BigEndian>(pipe_id));

		let mut hops = 0;
		loop {
			if hops >= self.ttl {
				return Ok(None);
			}

			hops += 1;

			if body.len() < 4 {
				return Ok(None);
			}

			let tail = body.split_off(4);
			header.push_all(&body);

			let position = header.len() - 4;
			if header[position] & 0x80 != 0 {
				let msg = Message::with_header_and_body(header, tail);

				return Ok(Some(msg));
			}
			body = tail;
		}

	}

	fn msg_to_raw_msg(&mut self, msg: Message) -> io::Result<(Message, mio::Token)> {
		let (mut header, body) = msg.explode();
		let pipe_id_bytes = vec!(
			self.backtrace[0],
			self.backtrace[1],
			self.backtrace[2],
			self.backtrace[3]
		);
		let mut pipe_id_reader = io::Cursor::new(pipe_id_bytes);
		let pipe_id = try!(pipe_id_reader.read_u32::<BigEndian>());
		let pipe_token = mio::Token(pipe_id as usize);

		self.restore_saved_backtrace_to_header(&mut header);
		self.backtrace.clear();

		Ok((Message::with_header_and_body(header, body), pipe_token))
	}

	fn restore_saved_backtrace_to_header(&self, header: &mut Vec<u8>) {
		let backtrace = &self.backtrace[4..];
		
		header.reserve(backtrace.len());
		header.push_all(backtrace);
	}

	fn on_raw_msg_recv(&mut self, event_loop: &mut EventLoop, pipe_token: mio::Token, raw_msg: Message) {
		match self.raw_msg_to_msg(pipe_token, raw_msg) {
			Ok(None)      => {}
			Ok(Some(msg)) => self.on_msg_recv_ok(event_loop, msg),
			Err(e)        => self.on_msg_recv_err(event_loop, e)
		}
	}
}

impl Protocol for Resp {
	fn id(&self) -> u16 {
		SocketType::Respondent.id()
	}

	fn peer_id(&self) -> u16 {
		SocketType::Surveyor.id()
	}

	fn add_pipe(&mut self, token: mio::Token, pipe: Pipe) {
		self.pipe = Some(RespPipe::new(token, pipe));
	}

	fn remove_pipe(&mut self, token: mio::Token) -> Option<Pipe> {
		if Some(token) == self.pipe.as_ref().map(|p| p.token()) {
			self.pipe.take().map(|p| p.remove())
		} else {
			None
		}
	}

	fn ready(&mut self, event_loop: &mut EventLoop, token: mio::Token, events: mio::EventSet) -> io::Result<()> {
		if self.pipe.is_none() {
			return Ok(());
		}
		if Some(token) != self.pipe.as_ref().map(|p| p.token()) {
			return Ok(());
		}

		let mut pipe = self.pipe.take().unwrap();
		let mut msg_sending = false;
		let mut msg_sent = false;
		let mut received_msg = None;
		let mut receiving_msg = false;

		let (sent, received) = try!(pipe.ready(event_loop, events));

		if sent {
			msg_sent = true;
		} else {
			match try!(pipe.resume_pending_send()) {
				Some(true)  => msg_sent = true,
				Some(false) => msg_sending = true,
				None        => {}
			}
		}

		if received.is_some() {
			received_msg = received;
		} else {
			match try!(pipe.resume_pending_recv()) {
				Some(RecvStatus::Completed(msg))   => received_msg = Some(msg),
				Some(RecvStatus::InProgress)       => receiving_msg = true,
				Some(RecvStatus::Postponed) | None => {}
			}
		}

		if msg_sent {
			self.on_msg_send_ok(event_loop);
		}

		if msg_sending | msg_sent {
			pipe.reset_pending_send();
		}

		if received_msg.is_some() | receiving_msg {
			pipe.reset_pending_recv();
		}

		if received_msg.is_some() {
			let raw_msg = received_msg.unwrap();

			self.on_raw_msg_recv(event_loop, token, raw_msg);
		}

		self.pipe = Some(pipe);

		Ok(())
	}

	fn send(&mut self, event_loop: &mut EventLoop, msg: Message, cancel_timeout: Box<FnBox(&mut EventLoop)-> bool>) {
		if self.pipe.is_none() {
			let err = io::Error::new(io::ErrorKind::NotConnected, "no connected endpoint");

			self.on_msg_send_err(event_loop, err);

			return;
		}

		let mut pipe = self.pipe.take().unwrap();

		self.cancel_send_timeout = Some(cancel_timeout);

		let (raw_msg, pipe_token) = match self.msg_to_raw_msg(msg) {
			Err(e) => {
				self.on_msg_send_err(event_loop, e);
				return;
			},
			Ok((raw_msg, pipe_token)) => (raw_msg, pipe_token)
		};

		if pipe_token != pipe.token() {
			let err = io::Error::new(io::ErrorKind::NotConnected, "original surveyor disconnected");

			self.on_msg_send_err(event_loop, err);

			return;
		}

		let mut sent = false;
		let mut sending = false;
		let mut pending = false;

		match pipe.send(Rc::new(raw_msg)) {
			Ok(Some(true))  => sent = true,
			Ok(Some(false)) => sending = true,
			Ok(None)        => pending = true,
			Err(_)          => {} 
			// this pipe looks dead, but it will be taken care of during next ready notification
		}

		if sent {
			self.on_msg_send_ok(event_loop);
		}

		if sent | sending {
			pipe.reset_pending_send();
		} else if pending == false {
			let err = io::Error::new(io::ErrorKind::NotConnected, "no connected endpoint");

			self.on_msg_send_err(event_loop, err);
		}

		self.pipe = Some(pipe);
	}

	fn on_send_timeout(&mut self, event_loop: &mut EventLoop) {
		let err = io::Error::new(io::ErrorKind::TimedOut, "send timeout reached");

		self.on_msg_send_err(event_loop, err);

		if let Some(pipe) = self.pipe.as_mut() {
			pipe.on_send_timeout();
		}
	}

	fn recv(&mut self, event_loop: &mut EventLoop, cancel_timeout: Box<FnBox(&mut EventLoop)-> bool>) {
		if self.pipe.is_none() {
			let err = io::Error::new(io::ErrorKind::NotConnected, "no connected endpoint");

			self.on_msg_send_err(event_loop, err);

			return;
		}

		let mut pipe = self.pipe.take().unwrap();

		self.cancel_recv_timeout = Some(cancel_timeout);

		let mut received = None;
		let mut receiving = false;
		let mut pending = false;

		match pipe.recv() {
			Ok(RecvStatus::Completed(msg)) => received = Some(msg),
			Ok(RecvStatus::InProgress)     => receiving = true,
			Ok(RecvStatus::Postponed)      => pending = true,
			Err(_)                         => {}
		}

		if received.is_some() | receiving {
			pipe.reset_pending_recv();
		} else if pending == false {
			let err = io::Error::new(io::ErrorKind::NotConnected, "no connected endpoint");

			self.on_msg_recv_err(event_loop, err);
		}

		if received.is_some() {
			let pipe_token = pipe.token();
			let raw_msg = received.unwrap();

			self.on_raw_msg_recv(event_loop, pipe_token, raw_msg);
		}

		self.pipe = Some(pipe);
	}

	fn on_recv_timeout(&mut self, event_loop: &mut EventLoop) {
		let err = io::Error::new(io::ErrorKind::TimedOut, "recv timeout reached");

		self.on_msg_recv_err(event_loop, err);

		if let Some(pipe) = self.pipe.as_mut() {
			pipe.on_recv_timeout();
		}
	}
}

struct RespPipe {
	token: mio::Token,
    pipe: Pipe,
    pending_send: Option<Rc<Message>>,
    pending_recv: bool
}

impl RespPipe {
	fn new(token: mio::Token, pipe: Pipe) -> RespPipe {
		RespPipe { 
			token: token,
			pipe: pipe,
			pending_send: None,
			pending_recv: false
		}
	}

	fn token(&self) -> mio::Token {
		self.token
	}

	fn ready(&mut self, event_loop: &mut EventLoop, events: mio::EventSet) -> io::Result<(bool, Option<Message>)> {
		self.pipe.ready(event_loop, events)
	}

	fn send(&mut self, msg: Rc<Message>) -> io::Result<Option<bool>> {
		let (pending_send, progress) = match try!(self.pipe.send(msg)) {
			SendStatus::Completed      => (None, Some(true)),
			SendStatus::InProgress     => (None, Some(false)),
			SendStatus::Postponed(msg) => (Some(msg), None)
		};

		self.pending_send = pending_send;

		Ok(progress)
	}

	fn on_send_timeout(&mut self) {
		self.pending_send = None;
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

	fn recv(&mut self) -> io::Result<RecvStatus> {
		let progress = try!(self.pipe.recv());

		self.pending_recv = match progress {
			RecvStatus::Completed(_) => false,
			RecvStatus::InProgress   => false,
			RecvStatus::Postponed    => true
		};

		Ok(progress)
	}

	fn on_recv_timeout(&mut self) {
		self.pending_recv = false;
		self.pipe.cancel_receiving();
	}

	fn resume_pending_recv(&mut self) -> io::Result<Option<RecvStatus>> {
		let result = if self.pending_recv {
			let status = try!(self.pipe.recv());
			Some(status)
		} else {
			None
		};

		Ok(result)
	}

	fn reset_pending_recv(&mut self) {
		self.pending_recv = false;
	}

	fn remove(self) -> Pipe {
		self.pipe
	}
}