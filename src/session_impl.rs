use std::sync::mpsc;

use mio;
use mio::util::Slab;

use global::SocketType as SocketType;

use event_loop_msg::EventLoopCmd as EventLoopCmd;
use event_loop_msg::SessionEvt as SessionEvt;
use event_loop_msg::SocketEvt as SocketEvt;

use socket_impl::SocketImpl as SocketImpl;

pub struct SessionImpl {
	event_sender: mpsc::Sender<SessionEvt>,
	sockets: Slab<SocketImpl>
}

impl SessionImpl {

	pub fn new(event_tx: mpsc::Sender<SessionEvt>, socket_count: usize) -> SessionImpl {
		SessionImpl {
			event_sender: event_tx,
			sockets: Slab::new(socket_count)
		}
	}

	fn pong(&self) {
		self.event_sender.send(SessionEvt::Pong);
	}

	fn create_socket(&mut self, socket_type: SocketType) {
		let (tx, rx) = mpsc::channel();
		let socket = SocketImpl::new(tx);

		let evt = match self.sockets.insert(socket) {
			Ok(id) => SessionEvt::SocketCreated(id.as_usize(), rx),
			Err(_) => SessionEvt::SocketNotCreated
		};

		self.event_sender.send(evt);
	}
	
	fn ping_socket(&mut self, id: usize) {
		let token = mio::Token(id);
		let socket = self.get_socket(token);

		socket.pong();
	}

    fn get_socket<'a>(&'a mut self, token: mio::Token) -> &'a mut SocketImpl {
        &mut self.sockets[token]
    }
}

impl mio::Handler for SessionImpl {
    type Timeout = ();
    type Message = EventLoopCmd;

    fn notify(&mut self, event_loop: &mut mio::EventLoop<Self>, msg: Self::Message) {
    	match msg {
    		EventLoopCmd::Ping => self.pong(),
    		EventLoopCmd::CreateSocket(socket_type) => self.create_socket(socket_type),
    		EventLoopCmd::PingSocket(id) => self.ping_socket(id),
    		EventLoopCmd::Shutdown => event_loop.shutdown()
    	}
    	
    }
}
