// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::io;

use mio;

use protocol::Protocol;
use protocol::policy::*;
use transport::pipe::Pipe;
use global::*;
use event_loop_msg::{ SocketNotify };
use EventLoop;
use Message;

pub struct Pub {
    notify_sender: Rc<Sender<SocketNotify>>,
    pipes: HashMap<mio::Token, Pipe>
}

impl Pub {
    pub fn new(_: SocketId, notify_tx: Rc<Sender<SocketNotify>>) -> Pub {
        Pub {
            notify_sender: notify_tx,
            pipes: HashMap::new()
        }
    }
}

impl Protocol for Pub {
    fn get_type(&self) -> SocketType {
        SocketType::Pub
    }

    fn add_pipe(&mut self, tok: mio::Token, pipe: Pipe) -> io::Result<()> {
        WithPipes::insert_into_pipes(self, tok, pipe)
     }

    fn remove_pipe(&mut self, tok: mio::Token) -> Option<Pipe> {
        self.pipes.remove(&tok)
    }

    fn open_pipe(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.pipes.get_mut(&tok).map(|p| p.open(event_loop));
    }

    fn on_pipe_opened(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.pipes.get_mut(&tok).map(|p| p.on_open_ack(event_loop));
    }

    fn send(&mut self, event_loop: &mut EventLoop, msg: Message, timeout: Timeout) {
        self.send_all(event_loop, Rc::new(msg), timeout);
    }

    fn on_send_done(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        WithBroadcast::on_send_done(self, event_loop, tok);
    }

    fn on_send_timeout(&mut self, _: &mut EventLoop) {
    }

    fn has_pending_send(&self) -> bool {
        false
    }

    fn recv(&mut self, event_loop: &mut EventLoop, timeout: Timeout) {
        WithoutRecv::recv(self);
        clear_timeout(event_loop, timeout);
    }

    fn on_recv_done(&mut self, _: &mut EventLoop, _: mio::Token, _: Message) {
    }

    fn on_recv_timeout(&mut self, _: &mut EventLoop) {
    }

    fn ready(&mut self, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) {
        self.get_pipe_mut(&tok).map(|p| p.ready(event_loop, events));
    }

    fn destroy(&mut self, event_loop: &mut EventLoop) {
        WithPipes::destroy_pipes(self, event_loop);
    }
}

impl WithNotify for Pub {
    fn get_notify_sender(&self) -> &Sender<SocketNotify> {
        &self.notify_sender
    }
}

impl WithPipes for Pub {
    fn get_pipes(&self) -> &HashMap<mio::Token, Pipe> {
        &self.pipes
    }
    fn get_pipes_mut(&mut self) -> &mut HashMap<mio::Token, Pipe> {
        &mut self.pipes
    }
}

impl WithBroadcast for Pub {
}

impl WithoutRecv for Pub {
}
