use std::cell::Cell;
use std::collections::HashMap;

use std::sync::mpsc;

use mio;
use mio::util::Slab;

use global::SocketType as SocketType;

use event_loop_msg::EventLoopCmd as EventLoopCmd;
use event_loop_msg::EventLoopTimeout as EventLoopTimeout;
use event_loop_msg::SessionEvt as SessionEvt;
use event_loop_msg::SocketEvt as SocketEvt;

use socket_impl::SocketImpl as SocketImpl;
use protocol;

use EventLoop;
use Message;

pub struct SessionImpl {
	event_sender: mpsc::Sender<SessionEvt>,
	sockets: HashMap<usize, SocketImpl>,
	socket_ids: HashMap<usize, usize>,
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

	fn pong(&self) {
		self.event_sender.send(SessionEvt::Pong);
	}

	fn create_socket(&mut self, socket_type: SocketType) {
		let protocol = protocol::create_protocol(socket_type);
		let (tx, rx) = mpsc::channel();
		let socket = SocketImpl::new(protocol, tx);
		let id = self.next_id();

		self.sockets.insert(id, socket);
		self.event_sender.send(SessionEvt::SocketCreated(id, rx));
	}
	
	fn ping_socket(&mut self, id: usize) {
		if let Some(socket) = self.sockets.get_mut(&id) {
			socket.pong();
		}
	}

	fn connect_socket(&mut self, id: usize, event_loop: &mut EventLoop, addr: &str) {
		let connection_id = self.next_id();

		self.socket_ids.insert(connection_id, id);

		if let Some(socket) = self.sockets.get_mut(&id) {
			socket.connect(addr, event_loop, connection_id);
		}
	}

	fn send_msg(&mut self, id: usize, event_loop: &mut EventLoop, msg: Message) {
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
    		EventLoopCmd::Ping => self.pong(),
    		EventLoopCmd::CreateSocket(socket_type) => self.create_socket(socket_type),
    		EventLoopCmd::PingSocket(id) => self.ping_socket(id),
    		EventLoopCmd::ConnectSocket(id, addr) => self.connect_socket(id, event_loop, &addr),
    		EventLoopCmd::SendMsg(id, msg) => self.send_msg(id, event_loop, msg),
    		EventLoopCmd::Shutdown => event_loop.shutdown()
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
						socket.connect(&addr, event_loop, id);
					}
				}
			}
		}
	}

    fn interrupted(&mut self, event_loop: &mut EventLoop) {

    }
}
