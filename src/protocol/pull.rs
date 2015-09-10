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

use super::receiver::*;

pub struct Pull {
    pipes: HashMap<mio::Token, Pipe>,
    evt_sender: Rc<mpsc::Sender<SocketEvt>>,
    msg_receiver: PolyadicMsgReceiver,
    codec: NoopMsgDecoder
}

impl Pull {
    pub fn new(evt_tx: Rc<mpsc::Sender<SocketEvt>>) -> Pull {
        Pull { 
            pipes: HashMap::new(),
            evt_sender: evt_tx.clone(),
            msg_receiver: PolyadicMsgReceiver::new(evt_tx),
            codec: NoopMsgDecoder
        }
    }
}

impl Protocol for Pull {
    fn id(&self) -> u16 {
        SocketType::Pull.id()
    }

    fn peer_id(&self) -> u16 {
        SocketType::Push.id()
    }

    fn add_endpoint(&mut self, token: mio::Token, endpoint: Endpoint) {
        self.pipes.insert(token, Pipe::new(token, endpoint));
    }

    fn remove_endpoint(&mut self, token: mio::Token) -> Option<Endpoint> {
        self.pipes.remove(&token).map(|p| p.remove())
    }

    fn send(&mut self, _: &mut EventLoop, _: Message, _: EventLoopAction) {
        let err = other_io_error("send not supported by protocol");
        let cmd = SocketEvt::MsgNotSent(err);
        let _ = self.evt_sender.send(cmd);
    }

    fn on_send_timeout(&mut self, _: &mut EventLoop) {
    }

    fn recv(&mut self, event_loop: &mut EventLoop, cancel_timeout: EventLoopAction) {
        self.msg_receiver.recv(event_loop, &mut self.codec, cancel_timeout, &mut self.pipes);
    }

    fn on_recv_timeout(&mut self, event_loop: &mut EventLoop) {
        self.msg_receiver.on_recv_timeout(event_loop, &mut self.pipes)
    }

    fn ready(&mut self, event_loop: &mut EventLoop, token: mio::Token, events: mio::EventSet) -> io::Result<()> {
        let mut received = None;

        if let Some(pipe) = self.pipes.get_mut(&token) {
            received = try!(pipe.ready_rx(event_loop, events));
        }

        match received {
            Some(msg) => Ok(self.msg_receiver.received_by(event_loop, &mut self.codec, msg, token, &mut self.pipes)),
            None => self.msg_receiver.resume_recv(event_loop, &mut self.codec, token, &mut self.pipes)
        }
    }
}
