// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::sync::mpsc;
use std::collections::HashMap;
use std::iter::Iterator;
use std::io;

use mio;

use super::Protocol;
use pipe::RecvStatus;
use endpoint::*;
use event_loop_msg::SocketEvt;
use EventLoop;
use EventLoopAction;
use Message;

type EndpointHashMap = HashMap<mio::Token, Endpoint>;

pub trait MsgDecoder {
    fn decode(&mut self, msg: Message, endpoint_token: mio::Token) -> io::Result<Option<Message>>;
}

pub struct NoopMsgDecoder;

impl MsgDecoder for NoopMsgDecoder {
    fn decode(&mut self, msg: Message, _: mio::Token) -> io::Result<Option<Message>> {
        Ok(Some(msg))
    }
}

pub struct PolyadicMsgReceiver {
    evt_sender: Rc<mpsc::Sender<SocketEvt>>,
    cancel_timeout: Option<EventLoopAction>,
    pending_recv: bool
}

impl PolyadicMsgReceiver {
    pub fn new(evt_tx: Rc<mpsc::Sender<SocketEvt>>) -> PolyadicMsgReceiver {
        PolyadicMsgReceiver {
            evt_sender: evt_tx,
            cancel_timeout: None,
            pending_recv: false
        }
    }

    pub fn recv(&mut self,
        event_loop: &mut EventLoop,
        decoder: &mut MsgDecoder,
        cancel_timeout: EventLoopAction,
        endpoints: &mut EndpointHashMap) {

        self.cancel_timeout = Some(cancel_timeout);

        let mut received = None;
        let mut receiving = None;
        for (_, endpoint) in endpoints.iter_mut() {
            match endpoint.recv() {
                Ok(RecvStatus::Completed(msg)) => received = Some((msg, endpoint.token())),
                Ok(RecvStatus::InProgress)     => receiving = Some(endpoint.token()),
                _ => continue
            }
            break;
        }

        self.pending_recv = true;
        self.process_recv_result(event_loop, decoder, received, receiving, endpoints);
    }

    pub fn received_by(&mut self,
        event_loop: &mut EventLoop,
        decoder: &mut MsgDecoder,
        raw_msg: Message, 
        token: mio::Token,
        endpoints: &mut EndpointHashMap) {

        self.process_recv_result(event_loop, decoder, Some((raw_msg, token)), None, endpoints);
    }

    pub fn resume_recv(&mut self,
        event_loop: &mut EventLoop,
        decoder: &mut MsgDecoder,
        token: mio::Token,
        endpoints: &mut EndpointHashMap) -> io::Result<()> {

        if self.pending_recv == false {
            return Ok(());
        }

        let mut received = None;
        let mut receiving = None;

        if let Some(endpoint) = endpoints.get_mut(&token) {
            if endpoint.can_resume_recv() {
                match try!(endpoint.recv()) {
                    RecvStatus::Completed(msg) => received = Some((msg, endpoint.token())),
                    RecvStatus::InProgress     => receiving = Some(token),
                    _ => {}
                }
            }
        }

        self.process_recv_result(event_loop, decoder, received, receiving, endpoints);

        Ok(())
    }

    pub fn on_recv_timeout(&mut self, event_loop: &mut EventLoop, endpoints: &mut EndpointHashMap) {
        let err = io::Error::new(io::ErrorKind::TimedOut, "recv timeout reached");

        self.on_msg_recv_finished_err(event_loop, err, endpoints);
    }

    pub fn on_recv_err(&mut self, event_loop: &mut EventLoop, err: io::Error, endpoints: &mut EndpointHashMap) {
        self.on_msg_recv_finished_err(event_loop, err, endpoints);
    }

    fn process_recv_result(&mut self,
        event_loop: &mut EventLoop,
        decoder: &mut MsgDecoder,
        received: Option<(Message, mio::Token)>,
        receiving: Option<mio::Token>,
        endpoints: &mut EndpointHashMap) {

        if let Some((raw_msg, tok)) = received {
            match decoder.decode(raw_msg, tok) {
                Err(e)        => self.on_msg_recv_finished_err(event_loop, e, endpoints),
                Ok(Some(msg)) => self.on_msg_recv_finished_ok(event_loop, msg, endpoints),
                Ok(None)      => {
                    if let Some(endpoint) = endpoints.get_mut(&tok) {
                        endpoint.finish_recv(); // msg was decoded and then discarded, try again
                    }
                }
            }
        }
        else if let Some(token) = receiving {
            self.on_msg_recv_started(token, endpoints);
        }
    }

    fn on_msg_recv_finished_ok(&mut self, event_loop: &mut EventLoop, msg: Message, endpoints: &mut EndpointHashMap) {
        self.on_msg_recv_finished(event_loop, SocketEvt::MsgRecv(msg));

        for (_, endpoint) in endpoints.iter_mut() {
            endpoint.finish_recv(); 
        }
    }

    fn on_msg_recv_finished_err(&mut self, event_loop: &mut EventLoop, err: io::Error, endpoints: &mut EndpointHashMap) {
        self.on_msg_recv_finished(event_loop, SocketEvt::MsgNotRecv(err));

        for (_, endpoint) in endpoints.iter_mut() {
            endpoint.cancel_recv(); 
        }
    }

    fn on_msg_recv_finished(&mut self, event_loop: &mut EventLoop, evt: SocketEvt) {
        let _ = self.evt_sender.send(evt);
        let timeout = self.cancel_timeout.take();

        timeout.map(|cancel_timeout| cancel_timeout.call_box((event_loop,)));

        self.pending_recv = false;
    }

    fn on_msg_recv_started(&mut self, token: mio::Token, endpoints: &mut EndpointHashMap) {
        self.pending_recv = false;
        for (_, endpoint) in endpoints.iter_mut() {
            if endpoint.token() == token {
                continue;
            } else {
                endpoint.discard_recv();
            }
        }
    }
}

pub struct UnaryMsgReceiver {
    evt_sender: Rc<mpsc::Sender<SocketEvt>>,
    cancel_timeout: Option<EventLoopAction>,
    pending_recv: bool
}

impl UnaryMsgReceiver {
    pub fn new(evt_tx: Rc<mpsc::Sender<SocketEvt>>) -> UnaryMsgReceiver {
        UnaryMsgReceiver {
            evt_sender: evt_tx,
            cancel_timeout: None,
            pending_recv: false
        }
    }

    pub fn recv(&mut self,
        event_loop: &mut EventLoop,
        decoder: &mut MsgDecoder,
        cancel_timeout: EventLoopAction,
        endpoint_cell: Option<&mut Endpoint>) {

        self.cancel_timeout = Some(cancel_timeout);
        self.pending_recv = true;

        if endpoint_cell.is_none() {
            return;
        }

        let mut endpoint = endpoint_cell.unwrap();
        let mut received = None;
        let mut receiving = None;
        match endpoint.recv() {
            Ok(RecvStatus::Completed(msg)) => received = Some((msg, endpoint.token())),
            Ok(RecvStatus::InProgress)     => receiving = Some(endpoint.token()),
            _ => {}
        }
        
        self.process_recv_result(event_loop, decoder, received, receiving, Some(endpoint));
    }

    pub fn received_by(&mut self,
        event_loop: &mut EventLoop,
        decoder: &mut MsgDecoder,
        raw_msg: Message, 
        token: mio::Token,
        endpoint_cell: Option<&mut Endpoint>) {

        self.process_recv_result(event_loop, decoder, Some((raw_msg, token)), None, endpoint_cell);
    }

    pub fn resume_recv(&mut self,
        event_loop: &mut EventLoop,
        decoder: &mut MsgDecoder,
        token: mio::Token,
        endpoint_cell: Option<&mut Endpoint>) -> io::Result<()> {

        if self.pending_recv == false {
            return Ok(());
        }

        if endpoint_cell.is_none() {
            return Ok(());
        }

        let mut endpoint = endpoint_cell.unwrap();
        let mut received = None;
        let mut receiving = None;

        if endpoint.can_resume_recv() {
            match try!(endpoint.recv()) {
                RecvStatus::Completed(msg) => received = Some((msg, token)),
                RecvStatus::InProgress     => receiving = Some(token),
                _ => {}
            }
        }

        self.process_recv_result(event_loop, decoder, received, receiving, Some(endpoint));

        Ok(())
    }

    pub fn on_recv_timeout(&mut self, event_loop: &mut EventLoop, endpoint_cell: Option<&mut Endpoint>) {
        let err = io::Error::new(io::ErrorKind::TimedOut, "recv timeout reached");

        self.on_msg_recv_finished_err(event_loop, err, endpoint_cell);
    }

    fn process_recv_result(&mut self,
        event_loop: &mut EventLoop,
        decoder: &mut MsgDecoder,
        received: Option<(Message, mio::Token)>,
        receiving: Option<mio::Token>,
        endpoint_cell: Option<&mut Endpoint>) {

        if let Some((raw_msg, tok)) = received {
            match decoder.decode(raw_msg, tok) {
                Err(e)        => self.on_msg_recv_finished_err(event_loop, e, endpoint_cell),
                Ok(Some(msg)) => self.on_msg_recv_finished_ok(event_loop, msg, endpoint_cell),
                Ok(None)      => {
                    if let Some(endpoint) = endpoint_cell {
                        endpoint.finish_recv(); // msg was decoded and then discarded, try again
                    }
                }
            }
        }
        else if receiving.is_some() {
            self.on_msg_recv_started();
        }
    }

    fn on_msg_recv_finished_ok(&mut self, event_loop: &mut EventLoop, msg: Message, endpoint_cell: Option<&mut Endpoint>) {
        self.on_msg_recv_finished(event_loop, SocketEvt::MsgRecv(msg));

        if let Some(endpoint) = endpoint_cell {
            endpoint.finish_recv(); 
        }
    }

    fn on_msg_recv_finished_err(&mut self, event_loop: &mut EventLoop, err: io::Error, endpoint_cell: Option<&mut Endpoint>) {
        self.on_msg_recv_finished(event_loop, SocketEvt::MsgNotRecv(err));

        if let Some(endpoint) = endpoint_cell {
            endpoint.cancel_recv(); 
        }
    }

    fn on_msg_recv_finished(&mut self, event_loop: &mut EventLoop, evt: SocketEvt) {
        let _ = self.evt_sender.send(evt);
        let timeout = self.cancel_timeout.take();

        timeout.map(|cancel_timeout| cancel_timeout.call_box((event_loop,)));

        self.pending_recv = false;
    }

    fn on_msg_recv_started(&mut self) {
        self.pending_recv = false;
    }
}