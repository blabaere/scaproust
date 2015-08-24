// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use mio;
use std::io;
use std::thread;
use std::sync::mpsc;

use event_loop_msg:: {
	EventLoopCmd,
	SessionCmd,
	SessionEvt,
	SocketEvt
};

use session_impl::SessionImpl;

use socket::Socket;
use global::*;

pub struct Session {
	cmd_sender: mio::Sender<EventLoopCmd>,
	evt_receiver: mpsc::Receiver<SessionEvt>
}

impl Session {
	pub fn new() -> io::Result<Session> {
	    let config = mio::EventLoopConfig {
            io_poll_timeout_ms: 250,
            notify_capacity: 4_096,
            messages_per_tick: 256,
            timer_tick_ms: 15,
            timer_wheel_size: 1_024,
            timer_capacity: 4_096
        };
		let mut event_loop = try!(mio::EventLoop::configured(config));
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

	fn send_cmd(&self, session_cmd: SessionCmd) -> Result<(), io::Error> {
		let cmd = EventLoopCmd::SessionLevel(session_cmd);

		self.cmd_sender.send(cmd).map_err(|e| convert_notify_err(e))
	}

	pub fn create_socket(&self, socket_type: SocketType) -> io::Result<Socket> {
		let session_cmd = SessionCmd::CreateSocket(socket_type);

		try!(self.send_cmd(session_cmd));

		match self.evt_receiver.recv() {
			Ok(SessionEvt::SocketCreated(id, rx)) => Ok(self.new_socket(id, rx)),
			Err(_)                                => Err(other_io_error("evt channel closed"))
		}
	}

	fn new_socket(&self, id: SocketId, rx: mpsc::Receiver<SocketEvt>) -> Socket {
		Socket::new(id, self.cmd_sender.clone(), rx)
	}
}

impl Drop for Session {
	fn drop(&mut self) {
		let session_cmd = SessionCmd::Shutdown;
		let cmd = EventLoopCmd::SessionLevel(session_cmd);
		
		let _ = self.cmd_sender.send(cmd);
	}
}

#[cfg(test)]
mod tests {
    use super::Session;
    use global::SocketType;

    #[test]
    fn session_can_create_a_socket() {
    	let session = Session::new().unwrap();
    	let socket = session.create_socket(SocketType::Push).unwrap();

    	drop(socket);
    }

    #[test]
    fn can_connect_socket() {
    	let session = Session::new().unwrap();
    	let mut socket = session.create_socket(SocketType::Push).unwrap();

    	assert!(socket.connect("tcp://127.0.0.1:5454").is_ok());
    }

    #[test]
    fn can_try_connect_socket() {
    	let session = Session::new().unwrap();
    	let mut socket = session.create_socket(SocketType::Push).unwrap();

    	assert!(socket.connect("tcp://this should not work").is_err());
    }
}