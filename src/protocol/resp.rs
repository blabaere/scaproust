// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::sync::mpsc::Sender;
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

pub struct Resp {
    pipe: Option<Pipe>,
    evt_sender: Rc<Sender<SocketEvt>>,
    cancel_send_timeout: Option<EventLoopAction>,
    cancel_recv_timeout: Option<EventLoopAction>,
    pending_send: Option<Rc<Message>>,
    pending_recv: bool,
    backtrace: Vec<u8>,
    ttl: u8
}

impl Resp {
    pub fn new(evt_sender: Rc<Sender<SocketEvt>>) -> Resp {
        Resp { 
            pipe: None,
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
    }

    fn on_msg_send_finished_ok(&mut self, event_loop: &mut EventLoop) {
        self.on_msg_send_finished(event_loop, SocketEvt::MsgSent);

        if let Some(pipe) = self.pipe.as_mut() {
            pipe.finish_send(); 
        }
    }

    fn on_msg_send_finished_err(&mut self, event_loop: &mut EventLoop, err: io::Error) {
        self.on_msg_send_finished(event_loop, SocketEvt::MsgNotSent(err));

        if let Some(pipe) = self.pipe.as_mut() {
            pipe.cancel_send(); 
        }
    }

    fn on_msg_send_started(&mut self) {
        self.pending_send = None;
    }

    fn process_send_result(&mut self, event_loop: &mut EventLoop, sent: bool, sending: bool) {
        if sent {
            self.on_msg_send_finished_ok(event_loop);
        } else if sending {
            self.on_msg_send_started();
        }
    }

    fn on_msg_recv_finished(&mut self, event_loop: &mut EventLoop, evt: SocketEvt) {
        let _ = self.evt_sender.send(evt);
        let timeout = self.cancel_recv_timeout.take();

        timeout.map(|cancel_timeout| cancel_timeout.call_box((event_loop,)));

        self.pending_recv = false;
    }

    fn on_msg_recv_finished_ok(&mut self, event_loop: &mut EventLoop, msg: Message) {
        self.save_received_header_to_backtrace(&msg);
        self.on_msg_recv_finished(event_loop, SocketEvt::MsgRecv(msg));

        if let Some(pipe) = self.pipe.as_mut() {
            pipe.finish_recv(); 
        }
    }

    fn on_msg_recv_finished_err(&mut self, event_loop: &mut EventLoop, err: io::Error) {
        self.on_msg_recv_finished(event_loop, SocketEvt::MsgNotRecv(err));

        if let Some(pipe) = self.pipe.as_mut() {
            pipe.cancel_recv(); 
        }
    }

    fn on_msg_recv_started(&mut self, _: mio::Token) {
        self.pending_recv = false;
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

    fn on_raw_msg_recv_from(&mut self, event_loop: &mut EventLoop, raw_msg: Message, pipe_token: mio::Token) {
        match self.raw_msg_to_msg(pipe_token, raw_msg) {
            Ok(None)      => {}
            Ok(Some(msg)) => self.on_msg_recv_finished_ok(event_loop, msg),
            Err(e)        => self.on_msg_recv_finished_err(event_loop, e)
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

                return Ok(Some(msg));
            }
            body = tail;
        }

    }

    fn msg_to_raw_msg(&mut self, msg: Message) -> io::Result<(Message, mio::Token)> {
        let (mut header, body) = msg.explode();
        let pipe_id_bytes = vec!(
            self.backtrace[0],
            self.backtrace[1],
            self.backtrace[2],
            self.backtrace[3]
        );
        let mut pipe_id_reader = io::Cursor::new(pipe_id_bytes);
        let pipe_id = try!(pipe_id_reader.read_u32::<BigEndian>());
        let pipe_token = mio::Token(pipe_id as usize);

        self.restore_saved_backtrace_to_header(&mut header);
        self.backtrace.clear();

        Ok((Message::with_header_and_body(header, body), pipe_token))
    }

    fn restore_saved_backtrace_to_header(&self, header: &mut Vec<u8>) {
        let backtrace = &self.backtrace[4..];
        
        header.reserve(backtrace.len());
        header.push_all(backtrace);
    }
}

impl Protocol for Resp {
    fn id(&self) -> u16 {
        SocketType::Respondent.id()
    }

    fn peer_id(&self) -> u16 {
        SocketType::Surveyor.id()
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
        self.cancel_send_timeout = Some(cancel_timeout);

        if self.pipe.is_none() {
            self.pending_send = Some(Rc::new(msg));
            return;
        }

        let (raw_msg, pipe_token) = match self.msg_to_raw_msg(msg) {
            Err(e) => return self.on_msg_send_finished_err(event_loop, e),
            Ok((raw_msg, pipe_token)) => (raw_msg, pipe_token)
        };

        let mut pipe = self.pipe.take().unwrap();
        let mut sent = false;
        let mut sending = false;
        let msg = Rc::new(raw_msg);

        if pipe_token != pipe.token() {
            let err = io::Error::new(io::ErrorKind::NotConnected, "original surveyor disconnected");

            self.on_msg_send_finished_err(event_loop, err);
            self.pipe = Some(pipe);

            return;
        }

        match pipe.send(msg.clone()) {
            Ok(SendStatus::Completed)  => sent = true,
            Ok(SendStatus::InProgress) => sending = true,
            _ => {}
        }

        self.pipe = Some(pipe);
        self.pending_send = Some(msg);
        self.process_send_result(event_loop, sent, sending);
    }

    fn on_send_timeout(&mut self, event_loop: &mut EventLoop) {
        let err = io::Error::new(io::ErrorKind::TimedOut, "send timeout reached");

        self.on_msg_send_finished_err(event_loop, err);
    }

    fn recv(&mut self, event_loop: &mut EventLoop, cancel_timeout: EventLoopAction) {
        self.cancel_recv_timeout = Some(cancel_timeout);

        if self.pipe.is_none() {
            self.pending_recv = true;
            return;
        }

        let mut pipe = self.pipe.take().unwrap();
        let mut received = None;
        let mut receiving = None;

        match pipe.recv() {
            Ok(RecvStatus::Completed(msg)) => received = Some((msg, pipe.token())),
            Ok(RecvStatus::InProgress)     => receiving = Some(pipe.token()),
            _ => {}
        }

        self.pipe = Some(pipe);
        self.pending_recv = true;
        self.process_recv_result(event_loop, received, receiving);
    }

    fn on_recv_timeout(&mut self, event_loop: &mut EventLoop) {
        let err = io::Error::new(io::ErrorKind::TimedOut, "recv timeout reached");

        self.on_msg_recv_finished_err(event_loop, err);
    }

    fn ready(&mut self, event_loop: &mut EventLoop, token: mio::Token, events: mio::EventSet) -> io::Result<()> {
        if self.pipe.is_none() {
            return Ok(());
        }
        if Some(token) != self.pipe.as_ref().map(|p| p.token()) {
            return Ok(());
        }
        let mut sent = false;
        let mut sending = false;
        let mut received = None;
        let mut receiving = None;

        if let Some(pipe) = self.pipe.as_mut() {
            let has_pending_send = self.pending_send.is_some();
            let has_pending_recv = self.pending_recv;
            let (s, r) = try!(pipe.ready(event_loop, events));
            sent = s;
            received = r.map(|msg| (msg, pipe.token()));

            if has_pending_send && !sent && pipe.can_resume_send() {
                let msg = self.pending_send.as_ref().unwrap();

                match try!(pipe.send(msg.clone())) {
                    SendStatus::Completed  => sent = true,
                    SendStatus::InProgress => sending = true,
                    _ => {}
                }
            }

            if has_pending_recv && received.is_none() && pipe.can_resume_recv() {
                match try!(pipe.recv()) {
                    RecvStatus::Completed(msg) => received = Some((msg, pipe.token())),
                    RecvStatus::InProgress     => receiving = Some(token),
                    _ => {}
                }
            }
        }

        self.process_send_result(event_loop, sent, sending);
        self.process_recv_result(event_loop, received, receiving);

        Ok(())
        /*let mut pipe = self.pipe.take().unwrap();
        let mut msg_sending = false;
        let mut msg_sent = false;
        let mut received_msg = None;
        let mut receiving_msg = false;

        let (sent, received) = try!(pipe.ready(event_loop, events));

        if sent {
            msg_sent = true;
        } else {
            match try!(pipe.resume_pending_send()) {
                Some(true)  => msg_sent = true,
                Some(false) => msg_sending = true,
                None        => {}
            }
        }

        if received.is_some() {
            received_msg = received;
        } else {
            match try!(pipe.resume_pending_recv()) {
                Some(RecvStatus::Completed(msg))   => received_msg = Some(msg),
                Some(RecvStatus::InProgress)       => receiving_msg = true,
                Some(RecvStatus::Postponed) | None => {}
            }
        }

        if msg_sent {
            self.on_msg_send_ok(event_loop);
        }

        if msg_sending | msg_sent {
            pipe.reset_pending_send();
        }

        if received_msg.is_some() | receiving_msg {
            pipe.reset_pending_recv();
        }

        if received_msg.is_some() {
            let raw_msg = received_msg.unwrap();

            self.on_raw_msg_recv(event_loop, token, raw_msg);
        }

        self.pipe = Some(pipe);
*/
    }
}
