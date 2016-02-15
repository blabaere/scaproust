// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::io;

use mio;

use super::{ Protocol, Timeout };
use super::clear_timeout;
use event_loop_msg::{ SocketNotify };
use EventLoop;
use Message;
use super::with_pipes::WithPipes;

pub trait WithUnicastSend : WithPipes {
    fn send(&mut self, event_loop: &mut EventLoop, msg: Rc<Message>, tok: mio::Token) -> bool {
        self.get_pipe_mut(&tok).map(|p| p.send(event_loop, msg)).is_some()
    }

    fn on_send_by_pipe(&mut self, event_loop: &mut EventLoop, timeout: Timeout) {
        self.send_notify(SocketNotify::MsgSent);

        clear_timeout(event_loop, timeout);
    }

    fn on_send_timeout(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        let err = io::Error::new(io::ErrorKind::TimedOut, "send timeout reached");

        self.send_notify(SocketNotify::MsgNotSent(err));
        self.get_pipe_mut(&tok).map(|p| p.cancel_send(event_loop));
    }
}