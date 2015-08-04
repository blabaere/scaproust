use std::rc::Rc;
use std::collections::HashMap;

use std::sync::mpsc;

use mio;

use global::*;
use event_loop_msg::*;

use socket_impl::SocketImpl as SocketImpl;
use protocol;

use EventLoop;
use Message;

pub struct SessionImpl {
	event_sender: mpsc::Sender<SessionEvt>,
	sockets: HashMap<SocketId, SocketImpl>,
	socket_ids: HashMap<mio::Token, SocketId>,
	id_seq: IdSequence
}

impl SessionImpl {

	pub fn new(event_tx: mpsc::Sender<SessionEvt>) -> SessionImpl {
		SessionImpl {
			event_sender: event_tx,
			sockets: HashMap::new(),
			socket_ids: HashMap::new(),
			id_seq: IdSequence::new()
		}
	}

	fn handle_session_cmd(&mut self, event_loop: &mut EventLoop, cmd: SessionCmd) {
		match cmd {
			SessionCmd::Ping => self.pong(),
			SessionCmd::CreateSocket(socket_type) => self.create_socket(socket_type),
			SessionCmd::Shutdown => event_loop.shutdown()
		}
	}

	fn handle_socket_cmd(&mut self, event_loop: &mut EventLoop, id: SocketId, cmd: SocketCmd) {
		match cmd {
			SocketCmd::Ping => self.ping_socket(id),
			SocketCmd::Connect(addr) => self.connect_socket(id, event_loop, addr),
			SocketCmd::Bind(addr) => self.bind_socket(id, event_loop, addr),
			SocketCmd::SendMsg(msg) => self.send(id, event_loop, msg)
		}
	}

	fn pong(&self) {
		self.event_sender.send(SessionEvt::Pong);
	}

	fn create_socket(&mut self, socket_type: SocketType) {
		let (tx, rx) = mpsc::channel();
		let shared_tx = Rc::new(tx);
		let protocol = protocol::create_protocol(socket_type, shared_tx.clone());
		let id = SocketId(self.id_seq.next());
		let socket = SocketImpl::new(id, protocol, shared_tx.clone(), self.id_seq.clone());

		self.sockets.insert(id, socket);
		self.event_sender.send(SessionEvt::SocketCreated(id, rx));
	}
	
	fn ping_socket(&mut self, id: SocketId) {
		if let Some(socket) = self.sockets.get_mut(&id) {
			socket.pong();
		}
	}

	fn connect_socket(&mut self, id: SocketId, event_loop: &mut EventLoop, addr: String) {
		let token = mio::Token(self.id_seq.next());

		self.socket_ids.insert(token, id);

		if let Some(socket) = self.sockets.get_mut(&id) {
			socket.connect(addr, event_loop, token);
		}
	}

	fn reconnect_socket(&mut self, event_loop: &mut EventLoop, token: mio::Token, addr: String) {
		if let Some(socket_id) = self.socket_ids.get_mut(&token) {
			if let Some(socket) = self.sockets.get_mut(&socket_id) {
				socket.reconnect(addr, event_loop, token);
			}
		}
	}

	fn bind_socket(&mut self, id: SocketId, event_loop: &mut EventLoop, addr: String) {
		let token = mio::Token(self.id_seq.next());

		self.socket_ids.insert(token, id);

		if let Some(socket) = self.sockets.get_mut(&id) {
			socket.bind(addr, event_loop, token);
		}
	}

	fn rebind_socket(&mut self, event_loop: &mut EventLoop, token: mio::Token, addr: String) {
		if let Some(socket_id) = self.socket_ids.get_mut(&token) {
			if let Some(socket) = self.sockets.get_mut(&socket_id) {
				socket.rebind(addr, event_loop, token);
			}
		}
	}

	fn send(&mut self, id: SocketId, event_loop: &mut EventLoop, msg: Message) {
		if let Some(socket) = self.sockets.get_mut(&id) {
			socket.send(event_loop, msg);
		}
	}

	fn on_send_timeout(&mut self, event_loop: &mut EventLoop, token: mio::Token) {
		let socket_id = SocketId(token.as_usize());
		if let Some(socket) = self.sockets.get_mut(&socket_id) {
			socket.on_send_timeout(event_loop);
		}
	}

    fn link(&mut self, mut tokens: Vec<mio::Token>, socket_id: SocketId) {
		for token in tokens.drain(..) {
			self.socket_ids.insert(token, socket_id);
		}
    }
}

impl mio::Handler for SessionImpl {
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
		debug!("timeout");

		match timeout {
			EventLoopTimeout::Reconnect(token, addr) => self.reconnect_socket(event_loop, token, addr),
			EventLoopTimeout::Rebind(token, addr)    => self.rebind_socket(event_loop, token, addr),
			EventLoopTimeout::CancelSend(token)      => self.on_send_timeout(event_loop, token)
		}
	}

    fn interrupted(&mut self, event_loop: &mut EventLoop) {

    }
}
