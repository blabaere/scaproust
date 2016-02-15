// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::io;

use mio;

use super::{ Protocol, Timeout };
use super::clear_timeout;
use event_loop_msg::{ SocketNotify };
use EventLoop;
use Message;
use super::with_pipes::WithPipes;

pub trait WithUnicastRecv : WithPipes {

    fn recv(&mut self, event_loop: &mut EventLoop, tok: mio::Token) -> bool {
        self.get_pipe(&tok).map(|p| p.recv(event_loop)).is_some()
    }

    fn on_recv_by_pipe(&mut self, event_loop: &mut EventLoop, msg: Message, timeout: Timeout) {
        self.send_notify(SocketNotify::MsgRecv(msg));
        
        clear_timeout(event_loop, timeout);
    }

    fn on_recv_timeout(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        let err = io::Error::new(io::ErrorKind::TimedOut, "recv timeout reached");

        self.send_notify(SocketNotify::MsgNotRecv(err));
        self.get_pipe(&tok).map(|p| p.cancel_recv(event_loop));
    }
}