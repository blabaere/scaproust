// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::sync::mpsc;
use std::collections::HashMap;
use std::io;
use std::boxed::FnBox;

use time;

use mio;

use byteorder::{ BigEndian, WriteBytesExt, ReadBytesExt };

use super::Protocol;
use pipe::*;
use endpoint::*;
use global::*;
use event_loop_msg::SocketEvt;
use EventLoop;
use EventLoopAction;
use Message;

use super::sender::SendToAny;

pub struct Req {
    pipes: HashMap<mio::Token, Pipe>,
    evt_sender: Rc<mpsc::Sender<SocketEvt>>,
    cancel_recv_timeout: Option<EventLoopAction>,
    pending_recv: bool,

    pending_req_id: Option<u32>,
    req_id_seq: u32,
    resend_interval: Option<u64>,
    msg_sender: SendToAny

}

impl Req {
    pub fn new(evt_sender: Rc<mpsc::Sender<SocketEvt>>) -> Req {
        Req { 
            pipes: HashMap::new(),
            evt_sender: evt_sender.clone(),
            cancel_recv_timeout: None,
            pending_recv: false,
            pending_req_id: None,
            req_id_seq: time::get_time().nsec as u32,
            resend_interval: Some(60_000),
            msg_sender: SendToAny::new(evt_sender)
        }
    }

    // what to do with 
    // self.pending_req_id = None;
    // ???

    /*fn on_msg_send_finished_err(&mut self, event_loop: &mut EventLoop, err: io::Error) {
        self.pending_req_id = None;
        self.on_msg_send_finished(event_loop, SocketEvt::MsgNotSent(err));
        for (_, pipe) in self.pipes.iter_mut() {
            pipe.cancel_send();
        }
    }*/

    fn on_msg_recv_finished(&mut self, event_loop: &mut EventLoop, evt: SocketEvt) {
        let _ = self.evt_sender.send(evt);
        let timeout = self.cancel_recv_timeout.take();

        timeout.map(|cancel_timeout| cancel_timeout.call_box((event_loop,)));

        self.pending_recv = false;
    }

    fn on_msg_recv_finished_ok(&mut self, event_loop: &mut EventLoop, msg: Message) {
        self.on_msg_recv_finished(event_loop, SocketEvt::MsgRecv(msg));

        for (_, pipe) in self.pipes.iter_mut() {
            pipe.finish_recv(); 
        }
    }

    fn on_msg_recv_finished_err(&mut self, event_loop: &mut EventLoop, err: io::Error) {
        self.on_msg_recv_finished(event_loop, SocketEvt::MsgNotRecv(err));

        for (_, pipe) in self.pipes.iter_mut() {
            pipe.cancel_recv(); 
        }
    }

    fn on_msg_recv_started(&mut self, token: mio::Token) {
        self.pending_recv = false;
        for (_, pipe) in self.pipes.iter_mut() {
            if pipe.token() == token {
                continue;
            } else {
                pipe.discard_recv();
            }
        }
    }

    fn process_recv_result(&mut self, event_loop: &mut EventLoop, received: Option<Message>, receiving: Option<mio::Token>) {
        if let Some(raw_msg) = received {
            self.on_raw_msg_recv(event_loop, raw_msg);
        }
        else if let Some(token) = receiving {
            self.on_msg_recv_started(token);
        }
    }

    fn next_req_id(&mut self) -> u32 {
        let next_id = self.req_id_seq | 0x80000000;

        self.req_id_seq += 1;

        next_id
    }

    fn raw_msg_to_msg(&mut self, raw_msg: Message) -> io::Result<(Message, u32)> {
        let (mut header, mut payload) = raw_msg.explode();
        let body = payload.split_off(4);
        let mut req_id_reader = io::Cursor::new(payload);

        let req_id = try!(req_id_reader.read_u32::<BigEndian>());

        if header.len() == 0 {
            header = req_id_reader.into_inner();
        } else {
            let req_id_bytes = req_id_reader.into_inner();
            header.push_all(&req_id_bytes);
        }

        Ok((Message::with_header_and_body(header, body), req_id))
    }

    fn msg_to_raw_msg(&mut self, msg: Message, req_id: u32) -> io::Result<Message> {
        let mut raw_msg = msg;

        raw_msg.header.reserve(4);
        try!(raw_msg.header.write_u32::<BigEndian>(req_id));

        Ok(raw_msg)
    }

    fn on_raw_msg_recv(&mut self, event_loop: &mut EventLoop, raw_msg: Message) {
        if raw_msg.get_body().len() < 4 {
            // TODO try to recv again ?
            return;
        }

        match self.raw_msg_to_msg(raw_msg) {
            Ok((msg, req_id)) => {
                let expected_id = self.pending_req_id.take().unwrap();

                if req_id == expected_id {
                    self.on_msg_recv_finished_ok(event_loop, msg);
                } else {
                    // TODO try to recv again ?
                    self.pending_req_id = Some(expected_id);
                }
            },
            Err(e) => {
                self.on_msg_recv_finished_err(event_loop, e);
            }
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
        let req_id = self.next_req_id();

        self.pending_req_id = Some(req_id);

        match self.msg_to_raw_msg(msg, req_id) {
            Err(err)    => self.msg_sender.on_send_err(event_loop, err, &mut self.pipes),
            Ok(raw_msg) => self.msg_sender.send(event_loop, raw_msg, cancel_timeout, &mut self.pipes)
        }
    }

    fn on_send_timeout(&mut self, event_loop: &mut EventLoop) {
        self.msg_sender.on_send_timeout(event_loop, &mut self.pipes);
    }

    fn recv(&mut self, event_loop: &mut EventLoop, cancel_timeout: EventLoopAction) {
        if self.pending_req_id.is_none() {
            return self.on_msg_recv_finished_err(event_loop, other_io_error("no pending request sent"));
        }

        self.cancel_recv_timeout = Some(cancel_timeout);

        let mut received = None;
        let mut receiving = None;
        for (_, pipe) in self.pipes.iter_mut() {
            match pipe.recv() {
                Ok(RecvStatus::Completed(msg)) => received = Some(msg),
                Ok(RecvStatus::InProgress)     => receiving = Some(pipe.token()),
                _ => continue
            }
            break;
        }

        self.pending_recv = true;
        self.process_recv_result(event_loop, received, receiving);
    }

    fn on_recv_timeout(&mut self, event_loop: &mut EventLoop) {
        let err = io::Error::new(io::ErrorKind::TimedOut, "recv timeout reached");

        self.on_msg_recv_finished_err(event_loop, err);
    }

    fn ready(&mut self, event_loop: &mut EventLoop, token: mio::Token, events: mio::EventSet) -> io::Result<()> {
        let mut sent = false;
        let mut received = None;
        let mut receiving = None;

        if let Some(pipe) = self.pipes.get_mut(&token) {
            let has_pending_recv = self.pending_recv;

            let (s, r) = try!(pipe.ready(event_loop, events));
            sent = s;
            received = r;

            if has_pending_recv && received.is_none() && pipe.can_resume_recv() {
                match try!(pipe.recv()) {
                    RecvStatus::Completed(msg) => received = Some(msg),
                    RecvStatus::InProgress     => receiving = Some(pipe.token()),
                    _ => {}
                }
            }
        }

        try!(self.msg_sender.on_pipe_ready(event_loop, token, sent, &mut self.pipes));
        self.process_recv_result(event_loop, received, receiving);

        Ok(())
    }
}
