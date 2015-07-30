use std::rc::Rc;
use std::cell::Cell;
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
	socket_ids: HashMap<usize, SocketId>,
	id_seq: Cell<usize>
}

impl SessionImpl {

	pub fn new(event_tx: mpsc::Sender<SessionEvt>) -> SessionImpl {
		SessionImpl {
			event_sender: event_tx,
			sockets: HashMap::new(),
			socket_ids: HashMap::new(),
			id_seq: Cell::new(1)
		}
	}

	fn next_id(&self) -> usize {
		let id = self.id_seq.get();

		self.id_seq.set(id+1);

		id
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
		let id = SocketId(self.next_id());
		let socket = SocketImpl::new(id, protocol, shared_tx.clone());

		self.sockets.insert(id, socket);
		self.event_sender.send(SessionEvt::SocketCreated(id, rx));
	}
	
	fn ping_socket(&mut self, id: SocketId) {
		if let Some(socket) = self.sockets.get_mut(&id) {
			socket.pong();
		}
	}

	fn connect_socket(&mut self, id: SocketId, event_loop: &mut EventLoop, addr: String) {
		let connection_id = self.next_id();

		self.socket_ids.insert(connection_id, id);

		if let Some(socket) = self.sockets.get_mut(&id) {
			socket.connect(addr, event_loop, connection_id);
		}
	}

	fn bind_socket(&mut self, id: SocketId, event_loop: &mut EventLoop, addr: String) {
		let connection_id = self.next_id();

		self.socket_ids.insert(connection_id, id);

		if let Some(socket) = self.sockets.get_mut(&id) {
			socket.bind(addr, event_loop, connection_id);
		}
	}


	fn send(&mut self, id: SocketId, event_loop: &mut EventLoop, msg: Message) {
		if let Some(socket) = self.sockets.get_mut(&id) {
			socket.send(event_loop, msg);
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
    	let id = token.as_usize();
    	if let Some(socket_id) = self.socket_ids.get_mut(&id) {
	    	if let Some(socket) = self.sockets.get_mut(&socket_id) {
				socket.ready(event_loop, id, events);
			}
		}    	
    }

    /*fn readable(&mut self, event_loop: &mut EventLoop, token: mio::Token, hint: mio::ReadHint) {
    	let id = token.as_usize();
    	if let Some(socket_id) = self.socket_ids.get_mut(&id) {
	    	if let Some(socket) = self.sockets.get_mut(&socket_id) {
				socket.readable(event_loop, id, hint);
			}
		}
    }

    fn writable(&mut self, event_loop: &mut EventLoop, token: mio::Token) {
    	let id = token.as_usize();
    	if let Some(socket_id) = self.socket_ids.get_mut(&id) {
	    	if let Some(socket) = self.sockets.get_mut(&socket_id) {
				socket.writable(event_loop, id);
			}
		}
    }*/

	fn timeout(&mut self, event_loop: &mut EventLoop, timeout: Self::Timeout) {
		debug!("timeout");

		match timeout {
			EventLoopTimeout::Reconnect(id, addr) => {
				if let Some(socket_id) = self.socket_ids.get_mut(&id) {
					if let Some(socket) = self.sockets.get_mut(&socket_id) {
						socket.reconnect(addr, event_loop, id);
					}
				}
			}
		}
	}

    fn interrupted(&mut self, event_loop: &mut EventLoop) {

    }
}
