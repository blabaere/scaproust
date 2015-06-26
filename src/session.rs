use mio;
use mio::util::Slab;
use std::io;
use std::thread;
use std::sync::mpsc;

pub struct Session {
	cmd_sender: mio::Sender<SessionCmd>,
	evt_receiver: mpsc::Receiver<SessionEvt>
	// Could use https://github.com/polyfractal/bounded-spsc-queue ?
	// Only if there is one receiver per NanoSocket and they are only 'Send'
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
		self.cmd_sender.send(SessionCmd::Ping);
		self.evt_receiver.recv().unwrap();
	}

	pub fn create_socket(&self) -> Option<Socket> {
		self.cmd_sender.send(SessionCmd::CreateSocket);
		match self.evt_receiver.recv().unwrap() {

			SessionEvt::SocketCreated(rx) => {

				let socket = Socket {
					cmd_sender: self.cmd_sender.clone(),
					evt_receiver: rx
				};

				Some(socket)
			}
			_ => None
		}
	}
}

impl Drop for Session {
	fn drop(&mut self) {
		self.cmd_sender.send(SessionCmd::Shutdown);
	}
}

pub struct Socket {
	cmd_sender: mio::Sender<SessionCmd>,
	evt_receiver: mpsc::Receiver<SocketEvt>
}

impl Socket {
	pub fn ping(&self) {
		self.cmd_sender.send(SessionCmd::PingSocket);
		self.evt_receiver.recv().unwrap();
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

enum SocketEvt {
    Pong
}

enum SessionCmd {
	Ping,
	CreateSocket,
	PingSocket,
	Shutdown
}

enum SessionEvt {
	Pong,
	SocketCreated(mpsc::Receiver<SocketEvt>)
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

		self.sockets.insert(socket);
		self.event_sender.send(SessionEvt::SocketCreated(rx));
	}
	
	fn ping_socket(&self) {
		for socket in self.sockets.iter() {
			socket.pong();
		}
	}
}

impl mio::Handler for SessionEventLoopHandler {
    type Timeout = ();
    type Message = SessionCmd;

    fn notify(&mut self, event_loop: &mut mio::EventLoop<Self>, msg: Self::Message) {
    	match msg {
    		SessionCmd::Ping => self.pong(),
    		SessionCmd::CreateSocket => self.create_socket(),
    		SessionCmd::PingSocket => self.ping_socket(),
    		SessionCmd::Shutdown => event_loop.shutdown()
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