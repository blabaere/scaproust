// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::sync::mpsc;
use std::collections::HashMap;
use std::io;

use time;

use mio;

use byteorder::{ BigEndian, WriteBytesExt, ReadBytesExt };

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

pub struct Req {
    pipes: HashMap<mio::Token, Pipe>,
    msg_sender: PolyadicMsgSender<UnicastSendingStrategy>,
    msg_receiver: PolyadicMsgReceiver,
    codec: Codec,
    resend_interval: Option<u64>
}

impl Req {
    pub fn new(evt_tx: Rc<mpsc::Sender<SocketEvt>>) -> Req {
        Req { 
            pipes: HashMap::new(),
            msg_sender: new_unicast_msg_sender(evt_tx.clone()),
            msg_receiver: PolyadicMsgReceiver::new(evt_tx.clone()),
            codec: Codec::new(),
            resend_interval: Some(60_000)
        }
    }
}

impl Protocol for Req {
    fn id(&self) -> u16 {
        SocketType::Req.id()
    }

    fn peer_id(&self) -> u16 {
        SocketType::Rep.id()
    }

    fn add_endpoint(&mut self, token: mio::Token, endpoint: Endpoint) {
        self.pipes.insert(token, Pipe::new(token, endpoint));
    }

    fn remove_endpoint(&mut self, token: mio::Token) -> Option<Endpoint> {
        self.pipes.remove(&token).map(|p| p.remove())
    }

    fn send(&mut self, event_loop: &mut EventLoop, msg: Message, cancel_timeout: EventLoopAction) {
        match self.codec.encode(msg) {
            Err(e) => self.msg_sender.on_send_err(event_loop, e, &mut self.pipes),
            Ok(raw_msg) => self.msg_sender.send(event_loop, raw_msg, cancel_timeout, &mut self.pipes)
        };
    }

    fn on_send_timeout(&mut self, event_loop: &mut EventLoop) {
        self.codec.clear_pending_request();
        self.msg_sender.on_send_timeout(event_loop, &mut self.pipes);
    }

    fn recv(&mut self, event_loop: &mut EventLoop, cancel_timeout: EventLoopAction) {
        match self.codec.has_pending_request() {
            true  => self.msg_receiver.recv(event_loop, &mut self.codec, cancel_timeout, &mut self.pipes),
            false => self.msg_sender.on_send_err(event_loop, other_io_error("no pending request sent"), &mut self.pipes)
        }
    }

    fn on_recv_timeout(&mut self, event_loop: &mut EventLoop) {
        self.msg_receiver.on_recv_timeout(event_loop, &mut self.pipes)
    }

    fn ready(&mut self, event_loop: &mut EventLoop, token: mio::Token, events: mio::EventSet) -> io::Result<()> {
        let mut sent = false;
        let mut received = None;

        if let Some(pipe) = self.pipes.get_mut(&token) {
            let (s, r) = try!(pipe.ready(event_loop, events));
            sent = s;
            received = r;
        }

        let send_result = match sent {
            true  => Ok(self.msg_sender.sent_by(event_loop, token, &mut self.pipes)),
            false => self.msg_sender.resume_send(event_loop, token, &mut self.pipes)
        };

        let recv_result = match received {
            Some(msg) => Ok(self.msg_receiver.received_by(event_loop, &mut self.codec, msg, token, &mut self.pipes)),
            None => self.msg_receiver.resume_recv(event_loop, &mut self.codec, token, &mut self.pipes)
        };

        send_result.and(recv_result)
    }

    fn set_option(&mut self, _: &mut EventLoop, _: SocketOption) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::InvalidData, "option not supported by protocol"))
    }

    fn on_survey_timeout(&mut self, _: &mut EventLoop) {}
    fn on_request_timeout(&mut self, _: &mut EventLoop) {}
}

struct Codec {
    pending_req_id: Option<u32>,
    req_id_seq: u32
}

impl Codec {
    fn new() -> Codec {
        Codec {
            pending_req_id: None,
            req_id_seq: time::get_time().nsec as u32
        }
    }
    
    fn encode(&mut self, msg: Message) -> io::Result<Message> {
        let mut raw_msg = msg;
        let req_id = self.next_req_id();

        self.pending_req_id = Some(req_id);
        
        raw_msg.header.reserve(4);
        try!(raw_msg.header.write_u32::<BigEndian>(req_id));

        Ok(raw_msg)
    }

    fn next_req_id(&mut self) -> u32 {
        let next_id = self.req_id_seq | 0x80000000;

        self.req_id_seq += 1;

        next_id
    }

    fn has_pending_request(&self) -> bool {
        self.pending_req_id.is_some()
    }

    fn clear_pending_request(&mut self) {
        self.pending_req_id = None;
    }
}

impl MsgDecoder for Codec {
    fn decode(&mut self, raw_msg: Message, _: mio::Token) -> io::Result<Option<Message>> {
        if raw_msg.get_body().len() < 4 {
            return Ok(None);
        }

        let (mut header, mut payload) = raw_msg.explode();
        let body = payload.split_off(4);
        let mut req_id_reader = io::Cursor::new(payload);

        let req_id = try!(req_id_reader.read_u32::<BigEndian>());
        let expected_id = self.pending_req_id.take().unwrap();

        if header.len() == 0 {
            header = req_id_reader.into_inner();
        } else {
            let req_id_bytes = req_id_reader.into_inner();
            header.push_all(&req_id_bytes);
        }

        if req_id == expected_id {
            Ok(Some(Message::with_header_and_body(header, body)))
        } else {
            self.pending_req_id = Some(expected_id);
            Ok(None)
        }
    }
}