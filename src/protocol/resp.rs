// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::sync::mpsc::Sender;
use std::io;

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

pub struct Resp {
    pipe: Option<Pipe>,
    msg_sender: UnaryMsgSender,
    msg_receiver: UnaryMsgReceiver,
    codec: Codec
}

impl Resp {
    pub fn new(evt_tx: Rc<Sender<SocketEvt>>) -> Resp {
        Resp { 
            pipe: None,
            msg_sender: UnaryMsgSender::new(evt_tx.clone()),
            msg_receiver: UnaryMsgReceiver::new(evt_tx.clone()),
            codec: Codec::new()
        }
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
        match self.codec.encode(msg) {
            Err(e) => self.msg_sender.on_send_err(event_loop, e, self.pipe.as_mut()),
            Ok((raw_msg, _)) => self.msg_sender.send(event_loop, raw_msg, cancel_timeout, self.pipe.as_mut())
        }
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

struct Codec {
    backtrace: Vec<u8>,
    ttl: u8
}

impl Codec {
    fn new() -> Codec {
        Codec {
            backtrace: Vec::with_capacity(64),
            ttl: 8
        }
    }

    fn encode(&mut self, msg: Message) -> io::Result<(Message, mio::Token)> {
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

    fn save_received_header_to_backtrace(&mut self, msg: &Message) {
        self.backtrace.clear();
        self.backtrace.push_all(msg.get_header());
    }
}

impl MsgDecoder for Codec {
    fn decode(&mut self, raw_msg: Message, pipe_token: mio::Token) -> io::Result<Option<Message>> {
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
}