#![crate_name = "scaproust"]

#[macro_use] extern crate log;
extern crate byteorder;
extern crate mio;
extern crate time;

mod global;
mod event_loop_msg;
mod session;
mod session_impl;
mod socket;
mod socket_impl;
mod protocol;
mod transport;
mod pipe;

pub use session::Session;

pub use socket::Socket;

pub use global::SocketType;

type EventLoop = mio::EventLoop<session_impl::SessionImpl>;
