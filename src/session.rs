use mio;
use mio::util::Slab;
use std::io;
use std::thread;
use std::sync::mpsc;

use event_loop_msg::EventLoopCmd as EventLoopCmd;
use event_loop_msg::SessionEvt as SessionEvt;
use event_loop_msg::SocketEvt as SocketEvt;

use socket::Socket as Socket;

pub struct Session {
	cmd_sender: mio::Sender<EventLoopCmd>,
	evt_receiver: mpsc::Receiver<SessionEvt>
}

impl Session {
	pub fn new() -> io::Result<Session> {
		let mut event_loop = try!(mio::EventLoop::new());
		let (tx, rx) = mpsc::channel();
		let mut handler = SessionEventLoopHandler { 
			event_sender: tx,
			sockets: Slab::new(1024) };
		let session = Session { 
			cmd_sender: event_loop.channel(),
			evt_receiver: rx };

		thread::spawn(move || event_loop.run(&mut handler));

		Ok(session)
	}

	fn ping_event_loop(&self) {
		self.cmd_sender.send(EventLoopCmd::Ping);
		self.evt_receiver.recv().unwrap();
	}

	pub fn create_socket(&self) -> Option<Socket> {
		self.cmd_sender.send(EventLoopCmd::CreateSocket);

		match self.evt_receiver.recv().unwrap() {
			SessionEvt::SocketCreated(id, rx) => {
				let cmd_sender = self.cmd_sender.clone();
				let socket = Socket::new(id, cmd_sender, rx);

				Some(socket)
			}
			_ => None
		}
	}
}

impl Drop for Session {
	fn drop(&mut self) {
		self.cmd_sender.send(EventLoopCmd::Shutdown);
	}
}

struct ProtocolSocket {
	evt_sender: mpsc::Sender<SocketEvt> 
}

impl ProtocolSocket {

	fn pong(&self) {
		self.evt_sender.send(SocketEvt::Pong);
	}

}

struct SessionEventLoopHandler {
	event_sender: mpsc::Sender<SessionEvt>,
	sockets: Slab<ProtocolSocket>
}

impl SessionEventLoopHandler {

	fn pong(&self) {
		self.event_sender.send(SessionEvt::Pong);
	}

	fn create_socket(&mut self) {
		let (tx, rx) = mpsc::channel();
		let socket = ProtocolSocket { evt_sender: tx };

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

    fn get_socket<'a>(&'a mut self, token: mio::Token) -> &'a mut ProtocolSocket {
        &mut self.sockets[token]
    }
}

impl mio::Handler for SessionEventLoopHandler {
    type Timeout = ();
    type Message = EventLoopCmd;

    fn notify(&mut self, event_loop: &mut mio::EventLoop<Self>, msg: Self::Message) {
    	match msg {
    		EventLoopCmd::Ping => self.pong(),
    		EventLoopCmd::CreateSocket => self.create_socket(),
    		EventLoopCmd::PingSocket(id) => self.ping_socket(id),
    		EventLoopCmd::Shutdown => event_loop.shutdown()
    	}
    	
    }
}

#[cfg(test)]
mod tests {
    use super::Session;

    #[test]
    fn session_can_be_created() {
    	let session = Session::new().unwrap();

    	session.ping_event_loop();
    }

    #[test]
    fn session_can_create_a_socket() {
    	let session = Session::new().unwrap();
    	let socket = session.create_socket();

    }

    #[test]
    fn can_ping_socket() {
    	let session = Session::new().unwrap();
    	let socket = session.create_socket().unwrap();

    	socket.ping();
    }
}