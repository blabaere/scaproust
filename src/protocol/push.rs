// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::sync::mpsc;
use std::collections::HashMap;
use std::io;

use mio;

use super::Protocol;
use pipe::*;
use endpoint::*;
use global::*;
use event_loop_msg::SocketEvt;
use EventLoop;
use EventLoopAction;
use Message;

use super::sender::*;

pub struct Push {
    pipes: HashMap<mio::Token, Pipe>,
    evt_sender: Rc<mpsc::Sender<SocketEvt>>,
    msg_sender: PolyadicMsgSender<UnicastSendingStrategy>
}

impl Push {
    pub fn new(evt_sender: Rc<mpsc::Sender<SocketEvt>>) -> Push {
        Push { 
            pipes: HashMap::new(),
            evt_sender: evt_sender.clone(),
            msg_sender: new_unicast_msg_sender(evt_sender)
        }
    }
}

impl Protocol for Push {
    fn id(&self) -> u16 {
        SocketType::Push.id()
    }

    fn peer_id(&self) -> u16 {
        SocketType::Pull.id()
    }

    fn add_endpoint(&mut self, token: mio::Token, endpoint: Endpoint) {
        self.pipes.insert(token, Pipe::new(token, endpoint));
    }

    fn remove_endpoint(&mut self, token: mio::Token) -> Option<Endpoint> {
        self.pipes.remove(&token).map(|p| p.remove())
    }

    fn send(&mut self, event_loop: &mut EventLoop, msg: Message, cancel_timeout: EventLoopAction) {
        self.msg_sender.send(event_loop, msg, cancel_timeout, &mut self.pipes);
    }

    fn on_send_timeout(&mut self, event_loop: &mut EventLoop) {
        self.msg_sender.on_send_timeout(event_loop, &mut self.pipes);
    }

    fn recv(&mut self, _: &mut EventLoop, _: EventLoopAction) {
        let err = other_io_error("recv not supported by protocol");
        let cmd = SocketEvt::MsgNotRecv(err);
        let _ = self.evt_sender.send(cmd);
    }
    
    fn on_recv_timeout(&mut self, _: &mut EventLoop) {
    }

    fn ready(&mut self, event_loop: &mut EventLoop, token: mio::Token, events: mio::EventSet) -> io::Result<()> {
        let mut sent = false;

        if let Some(pipe) = self.pipes.get_mut(&token) {
            sent = try!(pipe.ready_tx(event_loop, events));
        }

        match sent {
            true  => Ok(self.msg_sender.sent_by(event_loop, token, &mut self.pipes)),
            false => self.msg_sender.resume_send(event_loop, token, &mut self.pipes)
        }
    }
}