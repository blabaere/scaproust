use mio;
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
		let mut handler = SessionEventLoopHandler { event_sender: tx };
		let session = Session { 
			cmd_sender: event_loop.channel(),
			evt_receiver: rx };

		thread::spawn(move || event_loop.run(&mut handler));

		Ok(session)
	}

	fn ping_event_loop(&self) {
		self.cmd_sender.send(SessionCmd::Ping);
		self.evt_receiver.recv();
	}
}

enum SessionCmd {
	Ping
}

enum SessionEvt {
	Pong
}

struct SessionEventLoopHandler {
	event_sender: mpsc::Sender<SessionEvt>
}

impl SessionEventLoopHandler {

	fn pong(&self) {
		self.event_sender.send(SessionEvt::Pong);
	}
	
}

impl mio::Handler for SessionEventLoopHandler {
    type Timeout = ();
    type Message = SessionCmd;

    fn notify(&mut self, event_loop: &mut mio::EventLoop<Self>, msg: Self::Message) {
    	match msg {
    		SessionCmd::Ping => self.pong()
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
}