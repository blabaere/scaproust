// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::collections::HashMap;

use std::sync::mpsc;

use mio;

use global::*;
use event_loop_msg::*;

use socket::Socket;
use protocol;

use EventLoop;
use Message;

pub struct Session {
	event_sender: mpsc::Sender<SessionEvt>,
	sockets: HashMap<SocketId, Socket>,
	socket_ids: HashMap<mio::Token, SocketId>,
	id_seq: IdSequence
}

impl Session {

	pub fn new(event_tx: mpsc::Sender<SessionEvt>) -> Session {
		Session {
			event_sender: event_tx,
			sockets: HashMap::new(),
			socket_ids: HashMap::new(),
			id_seq: IdSequence::new()
		}
	}

	fn handle_session_cmd(&mut self, event_loop: &mut EventLoop, cmd: SessionCmd) {
		match cmd {
			SessionCmd::CreateSocket(socket_type) => self.create_socket(socket_type),
			SessionCmd::Shutdown => event_loop.shutdown()
		}
	}

	fn handle_socket_cmd(&mut self, event_loop: &mut EventLoop, id: SocketId, cmd: SocketCmd) {
		match cmd {
			SocketCmd::Connect(addr)  => self.connect(event_loop, id, addr),
			SocketCmd::Bind(addr)     => self.bind(event_loop, id, addr),
			SocketCmd::SendMsg(msg)   => self.send(event_loop, id, msg),
			SocketCmd::RecvMsg        => self.recv(event_loop, id),
			SocketCmd::SetOption(opt) => self.set_option(event_loop, id, opt)
		}
	}

	fn send_evt(&self, evt: SessionEvt) {
		let send_res = self.event_sender.send(evt);

		if send_res.is_err() {
			error!("failed to notify event to session: '{:?}'", send_res.err());
		} 
	}

	fn create_socket(&mut self, socket_type: SocketType) {
		let (tx, rx) = mpsc::channel();
		let shared_tx = Rc::new(tx);
		let protocol = protocol::create_protocol(socket_type, shared_tx.clone());
		let id = SocketId(self.id_seq.next());
		let socket = Socket::new(id, protocol, shared_tx.clone(), self.id_seq.clone());

		self.sockets.insert(id, socket);

		self.send_evt(SessionEvt::SocketCreated(id, rx));
	}

	fn connect(&mut self, event_loop: &mut EventLoop, id: SocketId, addr: String) {
		let token = mio::Token(self.id_seq.next());

		self.socket_ids.insert(token, id);

		if let Some(socket) = self.sockets.get_mut(&id) {
			socket.connect(addr, event_loop, token);
		}
	}

	fn reconnect(&mut self, event_loop: &mut EventLoop, token: mio::Token, addr: String) {
		if let Some(socket_id) = self.socket_ids.get_mut(&token) {
			if let Some(socket) = self.sockets.get_mut(&socket_id) {
				socket.reconnect(addr, event_loop, token);
			}
		}
	}

	fn bind(&mut self, event_loop: &mut EventLoop, id: SocketId, addr: String) {
		let token = mio::Token(self.id_seq.next());

		self.socket_ids.insert(token, id);

		if let Some(socket) = self.sockets.get_mut(&id) {
			socket.bind(addr, event_loop, token);
		}
	}

	fn rebind(&mut self, event_loop: &mut EventLoop, token: mio::Token, addr: String) {
		if let Some(socket_id) = self.socket_ids.get_mut(&token) {
			if let Some(socket) = self.sockets.get_mut(&socket_id) {
				socket.rebind(addr, event_loop, token);
			}
		}
	}

	fn send(&mut self, event_loop: &mut EventLoop, id: SocketId, msg: Message) {
		if let Some(socket) = self.sockets.get_mut(&id) {
			socket.send(event_loop, msg);
		}
	}

	fn on_send_timeout(&mut self, event_loop: &mut EventLoop, socket_id: SocketId) {
		if let Some(socket) = self.sockets.get_mut(&socket_id) {
			socket.on_send_timeout(event_loop);
		}
	}

	fn recv(&mut self, event_loop: &mut EventLoop, id: SocketId) {
		if let Some(socket) = self.sockets.get_mut(&id) {
			socket.recv(event_loop);
		}
	}

	fn on_recv_timeout(&mut self, event_loop: &mut EventLoop, socket_id: SocketId) {
		if let Some(socket) = self.sockets.get_mut(&socket_id) {
			socket.on_recv_timeout(event_loop);
		}
	}

    fn link(&mut self, mut tokens: Vec<mio::Token>, socket_id: SocketId) {
		for token in tokens.drain(..) {
			self.socket_ids.insert(token, socket_id);
		}
    }

	fn set_option(&mut self, event_loop: &mut EventLoop, socket_id: SocketId, opt: SocketOption) {
		if let Some(socket) = self.sockets.get_mut(&socket_id) {
			socket.set_option(event_loop, opt);
		}
    }
}

impl mio::Handler for Session {
    type Timeout = EventLoopTimeout;
    type Message = EventLoopCmd;

    fn notify(&mut self, event_loop: &mut EventLoop, msg: Self::Message) {
    	debug!("session backend received a command");
    	match msg {
    		EventLoopCmd::SessionLevel(cmd) => self.handle_session_cmd(event_loop, cmd),
    		EventLoopCmd::SocketLevel(id, cmd) => self.handle_socket_cmd(event_loop, id, cmd)
    	}
    }

    fn ready(&mut self, event_loop: &mut EventLoop, token: mio::Token, events: mio::EventSet) {
    	let mut target_socket_id = None;
    	let mut added_tokens = None;

    	if let Some(socket_id) = self.socket_ids.get(&token) {
    		if let Some(socket) = self.sockets.get_mut(&socket_id) {
	    		target_socket_id = Some(socket_id.clone());
	    		added_tokens = socket.ready(event_loop, token, events);
			}
		}

    	if let Some(socket_id) = target_socket_id {
    		if let Some(tokens) = added_tokens {
    			self.link(tokens, socket_id);
    		}
    	}
    	
    }

	fn timeout(&mut self, event_loop: &mut EventLoop, timeout: Self::Timeout) {
		match timeout {
			EventLoopTimeout::Reconnect(token, addr) => self.reconnect(event_loop, token, addr),
			EventLoopTimeout::Rebind(token, addr)    => self.rebind(event_loop, token, addr),
			EventLoopTimeout::CancelSend(socket_id)  => self.on_send_timeout(event_loop, socket_id),
			EventLoopTimeout::CancelRecv(socket_id)  => self.on_recv_timeout(event_loop, socket_id)
		}
	}

    fn interrupted(&mut self, _: &mut EventLoop) {

    }
}
