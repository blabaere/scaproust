use mio;
use mio::util::Slab;
use std::io;
use std::thread;
use std::sync::mpsc;

use event_loop_msg::EventLoopCmd as EventLoopCmd;
use event_loop_msg::SessionEvt as SessionEvt;
use event_loop_msg::SocketEvt as SocketEvt;

use session_impl::SessionImpl as SessionImpl;

use socket::Socket as Socket;
use global::SocketType as SocketType;

pub struct Session {
	cmd_sender: mio::Sender<EventLoopCmd>,
	evt_receiver: mpsc::Receiver<SessionEvt>
}

impl Session {
	pub fn new() -> io::Result<Session> {
		let mut event_loop = try!(mio::EventLoop::new());
		let (tx, rx) = mpsc::channel();
		let mut handler = SessionImpl::new(tx, 1024);
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

	pub fn create_socket(&self, socket_type: SocketType) -> Option<Socket> {
		self.cmd_sender.send(EventLoopCmd::CreateSocket(socket_type));

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

#[cfg(test)]
mod tests {
    use super::Session;
    use global::SocketType;

    #[test]
    fn session_can_be_created() {
    	let session = Session::new().unwrap();

    	session.ping_event_loop();
    }

    #[test]
    fn session_can_create_a_socket() {
    	let session = Session::new().unwrap();
    	let socket = session.create_socket(SocketType::Push);

    }

    #[test]
    fn can_ping_socket() {
    	let session = Session::new().unwrap();
    	let socket = session.create_socket(SocketType::Push).unwrap();

    	socket.ping();
    }
}