use mio;
use std::io;
use std::thread;
use std::sync::mpsc;

use event_loop_msg:: {
	EventLoopCmd,
	SessionCmd,
	SessionEvt,
	SocketCmd,
	SocketEvt
};

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
		let session = Session { 
			cmd_sender: event_loop.channel(),
			evt_receiver: rx };

		thread::spawn(move || Session::run_event_loop(&mut event_loop, tx));

		Ok(session)
	}

	fn run_event_loop(event_loop: &mut mio::EventLoop<SessionImpl>, evt_tx: mpsc::Sender<SessionEvt>) {
		let mut handler = SessionImpl::new(evt_tx);
		let exec = event_loop.run(&mut handler);

		match exec {
			Ok(_) => debug!("event loop exited"),
			Err(e) => error!("event loop failed to run: {}", e)
		}
	}

	fn ping_event_loop(&self) {
		let session_cmd = SessionCmd::Ping;
		let cmd = EventLoopCmd::SessionLevel(session_cmd);

		self.cmd_sender.send(cmd);
		self.evt_receiver.recv().unwrap();
	}

	pub fn create_socket(&self, socket_type: SocketType) -> Option<Socket> {
		let session_cmd = SessionCmd::CreateSocket(socket_type);
		let cmd = EventLoopCmd::SessionLevel(session_cmd);
		self.cmd_sender.send(cmd);

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
		let session_cmd = SessionCmd::Shutdown;
		let cmd = EventLoopCmd::SessionLevel(session_cmd);
		
		self.cmd_sender.send(cmd);
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

    #[test]
    fn can_connect_socket() {
    	let session = Session::new().unwrap();
    	let socket = session.create_socket(SocketType::Push).unwrap();

    	socket.connect("tcp://127.0.0.1:5454").unwrap();
    }
}