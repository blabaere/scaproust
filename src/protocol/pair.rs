// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::sync::mpsc::Sender;
use std::io;

use mio;

use super::Protocol;
use pipe::*;
use endpoint::*;
use global::*;
use event_loop_msg::{ SocketEvt, SocketOption };
use EventLoop;
use EventLoopAction;
use Message;

use super::sender::*;
use super::receiver::*;

pub struct Pair {
    pipe: Option<Pipe>,
    msg_sender: UnaryMsgSender,
    msg_receiver: UnaryMsgReceiver,
    codec: NoopMsgDecoder
}

impl Pair {
    pub fn new(evt_tx: Rc<Sender<SocketEvt>>) -> Pair {
        Pair { 
            pipe: None,
            msg_sender: UnaryMsgSender::new(evt_tx.clone()),
            msg_receiver: UnaryMsgReceiver::new(evt_tx.clone()),
            codec: NoopMsgDecoder
        }
    }
}

impl Protocol for Pair {

    fn id(&self) -> u16 {
        SocketType::Pair.id()
    }

    fn peer_id(&self) -> u16 {
        SocketType::Pair.id()
    }

    fn add_endpoint(&mut self, token: mio::Token, endpoint: Endpoint) {
        self.pipe = Some(Pipe::new(token, endpoint));
    }

    fn remove_endpoint(&mut self, token: mio::Token) -> Option<Endpoint> {
        if Some(token) == self.pipe.as_ref().map(|p| p.token()) {
            self.pipe.take().map(|p| p.remove())
        } else {
            None
        }
    }

    fn send(&mut self, event_loop: &mut EventLoop, msg: Message, cancel_timeout: EventLoopAction) {
        self.msg_sender.send(event_loop, msg, cancel_timeout, self.pipe.as_mut())
    }

    fn on_send_timeout(&mut self, event_loop: &mut EventLoop) {
        self.msg_sender.on_send_timeout(event_loop, self.pipe.as_mut())
    }

    fn recv(&mut self, event_loop: &mut EventLoop, cancel_timeout: EventLoopAction) {
        self.msg_receiver.recv(event_loop, &mut self.codec, cancel_timeout, self.pipe.as_mut())
    }
    
    fn on_recv_timeout(&mut self, event_loop: &mut EventLoop) {
        self.msg_receiver.on_recv_timeout(event_loop, self.pipe.as_mut())
    }

    fn ready(&mut self, event_loop: &mut EventLoop, token: mio::Token, events: mio::EventSet) -> io::Result<()> {
        let mut sent = false;
        let mut received = None;

        if let Some(pipe) = self.pipe.as_mut() {
            let (s, r) = try!(pipe.ready(event_loop, events));
            sent = s;
            received = r;
        }

        let send_result = match sent {
            true  => Ok(self.msg_sender.sent_by(event_loop, token, self.pipe.as_mut())),
            false => self.msg_sender.resume_send(event_loop, token, self.pipe.as_mut())
        };

        let recv_result = match received {
            Some(msg) => Ok(self.msg_receiver.received_by(event_loop, &mut self.codec, msg, token, self.pipe.as_mut())),
            None => self.msg_receiver.resume_recv(event_loop, &mut self.codec, token, self.pipe.as_mut())
        };

        send_result.and(recv_result)
    }

    fn set_option(&mut self, _: &mut EventLoop, _: SocketOption) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::InvalidData, "option not supported by protocol"))
    }
}