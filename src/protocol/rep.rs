// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::sync::mpsc::Sender;
use std::collections::HashMap;
use std::io;
use std::boxed::FnBox;

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

pub struct Rep {
    pipes: HashMap<mio::Token, Pipe>,
    evt_sender: Rc<Sender<SocketEvt>>,
    cancel_send_timeout: Option<EventLoopAction>,
    cancel_recv_timeout: Option<EventLoopAction>,
    pending_send: Option<Rc<Message>>,
    pending_recv: bool,
    backtrace: Vec<u8>,
    ttl: u8
}

impl Rep {
    pub fn new(evt_sender: Rc<Sender<SocketEvt>>) -> Rep {
        Rep { 
            pipes: HashMap::new(),
            evt_sender: evt_sender,
            cancel_send_timeout: None,
            cancel_recv_timeout: None,
            pending_send: None,
            pending_recv: false,
            backtrace: Vec::with_capacity(64),
            ttl: 8
        }
    }

    fn on_msg_send_finished(&mut self, event_loop: &mut EventLoop, evt: SocketEvt) {
        let _ = self.evt_sender.send(evt);
        let timeout = self.cancel_send_timeout.take();

        timeout.map(|cancel_timeout| cancel_timeout.call_box((event_loop,)));

        self.pending_send = None;
        self.backtrace.clear();
    }

    fn on_msg_send_finished_ok(&mut self, event_loop: &mut EventLoop) {
        self.on_msg_send_finished(event_loop, SocketEvt::MsgSent);
        for (_, pipe) in self.pipes.iter_mut() {
            pipe.finish_send(); 
        }
    }

    fn on_msg_send_finished_err(&mut self, event_loop: &mut EventLoop, err: io::Error) {
        self.on_msg_send_finished(event_loop, SocketEvt::MsgNotSent(err));
        for (_, pipe) in self.pipes.iter_mut() {
            pipe.cancel_send();
        }
    }

    fn on_msg_send_started(&mut self, token: mio::Token) {
        self.pending_send = None;
        for (_, pipe) in self.pipes.iter_mut() {
            if pipe.token() == token {
                continue;
            } else {
                pipe.discard_send();
            }
        }
    }

    fn process_send_result(&mut self, event_loop: &mut EventLoop, sent: bool, sending: Option<mio::Token>) {
        if sent {
            self.on_msg_send_finished_ok(event_loop);
        } else if let Some(token) = sending {
            self.on_msg_send_started(token);
        }
    }

    fn save_received_header_to_backtrace(&mut self, msg: &Message) {
        self.backtrace.clear();
        self.backtrace.push_all(msg.get_header());
    }

    fn raw_msg_to_msg(&mut self, pipe_token: mio::Token, raw_msg: Message) -> io::Result<Option<Message>> {
        let (mut header, mut body) = raw_msg.explode();
        let pipe_id = pipe_token.as_usize() as u32;

        header.reserve(4);
        try!(header.write_u32::<BigEndian>(pipe_id));

        let mut hops = 0;
        loop {
            if hops >= self.ttl {
                return Ok(None);
            }

            hops += 1;

            if body.len() < 4 {
                return Ok(None);
            }

            let tail = body.split_off(4);
            header.push_all(&body);

            let position = header.len() - 4;
            if header[position] & 0x80 != 0 {
                let msg = Message::with_header_and_body(header, tail);

                self.save_received_header_to_backtrace(&msg);
                return Ok(Some(msg));
            }
            body = tail;
        }
    }

    fn msg_to_raw_msg(&mut self, msg: Message) -> io::Result<(Message, mio::Token)> {
        let (header, body) = msg.explode();
        let pipe_token = try!(self.restore_pipe_id_from_backtrace());
        let header = try!(self.restore_header_from_backtrace(header));

        Ok((Message::with_header_and_body(header, body), pipe_token))
    }

    fn restore_pipe_id_from_backtrace(&self) -> io::Result<mio::Token> {
        if self.backtrace.len() < 4 {
            return Err(other_io_error("no backtrace from received message"));
        }
        let pipe_id_bytes = vec!(
            self.backtrace[0],
            self.backtrace[1],
            self.backtrace[2],
            self.backtrace[3]
        );
        let mut pipe_id_reader = io::Cursor::new(pipe_id_bytes);
        let pipe_id = try!(pipe_id_reader.read_u32::<BigEndian>());
        let pipe_token = mio::Token(pipe_id as usize);

        Ok(pipe_token)
    }

    fn restore_header_from_backtrace(&self, mut header: Vec<u8>) -> io::Result<Vec<u8>> {
        if self.backtrace.len() < 8 {
            return Err(other_io_error("no header in backtrace from received message"));
        }

        let backtrace = &self.backtrace[4..];
        
        header.reserve(backtrace.len());
        header.push_all(backtrace);

        Ok(header)
    }

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

    fn on_raw_msg_recv_from(&mut self, event_loop: &mut EventLoop, raw_msg: Message, token: mio::Token) {
        match self.raw_msg_to_msg(token, raw_msg) {
            Ok(None)      => {}
            Ok(Some(msg)) => self.on_msg_recv_finished_ok(event_loop, msg),
            Err(e)        => self.on_msg_recv_finished_err(event_loop, e)
        }
    }

    fn process_recv_result(
        &mut self, 
        event_loop: &mut EventLoop, 
        received: Option<(Message, mio::Token)>, 
        receiving: Option<mio::Token>) {

        if let Some((raw_msg, from)) = received {
            self.on_raw_msg_recv_from(event_loop, raw_msg, from);
        }
        else if let Some(token) = receiving {
            self.on_msg_recv_started(token);
        }
    }
}

impl Protocol for Rep {
    fn id(&self) -> u16 {
        SocketType::Rep.id()
    }

    fn peer_id(&self) -> u16 {
        SocketType::Req.id()
    }

    fn add_endpoint(&mut self, token: mio::Token, endpoint: Endpoint) {
        self.pipes.insert(token, Pipe::new(token, endpoint));
    }

    fn remove_endpoint(&mut self, token: mio::Token) -> Option<Endpoint> {
        self.pipes.remove(&token).map(|p| p.remove())
    }

    fn send(&mut self, event_loop: &mut EventLoop, msg: Message, cancel_timeout: EventLoopAction) {
        self.cancel_send_timeout = Some(cancel_timeout);

        let (raw_msg, token) = match self.msg_to_raw_msg(msg) {
            Err(e) => return self.on_msg_send_finished_err(event_loop, e),
            Ok((raw_msg, token)) => (raw_msg, token)
        };

        let mut sent = false;
        let mut sending = None;
        let msg = Rc::new(raw_msg);

        if let Some(pipe) = self.pipes.get_mut(&token) {
            match pipe.send(msg.clone()) {
                Ok(SendStatus::Completed)  => sent = true,
                Ok(SendStatus::InProgress) => sending = Some(pipe.token()),
                _ => {}
            }
        }

        self.pending_send = Some(msg);
        self.process_send_result(event_loop, sent, sending);
    }

    fn on_send_timeout(&mut self, event_loop: &mut EventLoop) {
        let err = io::Error::new(io::ErrorKind::TimedOut, "send timeout reached");

        self.on_msg_send_finished_err(event_loop, err);
    }

    fn recv(&mut self, event_loop: &mut EventLoop, cancel_timeout: EventLoopAction) {
        self.cancel_recv_timeout = Some(cancel_timeout);

        let mut received = None;
        let mut receiving = None;
        for (_, pipe) in self.pipes.iter_mut() {
            match pipe.recv() {
                Ok(RecvStatus::Completed(msg)) => received = Some((msg, pipe.token())),
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
        let mut sending = None;
        let mut received = None;
        let mut receiving = None;

        if let Some(pipe) = self.pipes.get_mut(&token) {
            let has_pending_send = self.pending_send.is_some();
            let has_pending_recv = self.pending_recv;

            let (s, r) = try!(pipe.ready(event_loop, events));
            sent = s;
            received = r.map(|msg| (msg, pipe.token()));

            if has_pending_send && !sent && pipe.can_resume_send() {
                let msg = self.pending_send.as_ref().unwrap();

                match try!(pipe.send(msg.clone())) {
                    SendStatus::Completed  => sent = true,
                    SendStatus::InProgress => sending = Some(pipe.token()),
                    _ => {}
                }
            }

            if has_pending_recv && received.is_none() && pipe.can_resume_recv() {
                match try!(pipe.recv()) {
                    RecvStatus::Completed(msg) => received = Some((msg, pipe.token())),
                    RecvStatus::InProgress     => receiving = Some(pipe.token()),
                    _ => {}
                }
            }
        }

        self.process_send_result(event_loop, sent, sending);
        self.process_recv_result(event_loop, received, receiving);

        Ok(())
    }
}
