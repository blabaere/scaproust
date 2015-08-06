use std::rc::Rc;
use std::sync::mpsc;
use std::io;
use std::boxed::FnBox;

use mio;

use global::SocketType as SocketType;
use event_loop_msg::SocketEvt;
use pipe::Pipe as Pipe;
use EventLoop;
use Message;

pub mod push;
pub mod pull;
pub mod pair;

pub trait Protocol {
	fn id(&self) -> u16;
	fn peer_id(&self) -> u16;

	fn add_pipe(&mut self, token: mio::Token, pipe: Pipe);
	fn remove_pipe(&mut self, token: mio::Token) -> Option<Pipe>;

	fn ready(&mut self, event_loop: &mut EventLoop, token: mio::Token, events: mio::EventSet) -> io::Result<()>;

	fn send(&mut self, event_loop: &mut EventLoop, msg: Message, cancel_timeout: Box<FnBox(&mut EventLoop)-> bool>);
	fn on_send_timeout(&mut self, event_loop: &mut EventLoop);

	fn recv(&mut self, event_loop: &mut EventLoop, cancel_timeout: Box<FnBox(&mut EventLoop)-> bool>);
	fn on_recv_timeout(&mut self, event_loop: &mut EventLoop);
}


pub fn create_protocol(socket_type: SocketType, evt_tx: Rc<mpsc::Sender<SocketEvt>>) -> Box<Protocol> {
	match socket_type {
		SocketType::Push => Box::new(push::Push::new(evt_tx)),
		SocketType::Pull => Box::new(pull::Pull::new(evt_tx)),
		SocketType::Pair => Box::new(pair::Pair::new(evt_tx)),
		_ => panic!("")
	}
}
