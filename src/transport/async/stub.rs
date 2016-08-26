// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::ops::Deref;
use std::rc::Rc;
use std::io::{Result, Read, Write};

use byteorder::{ BigEndian, ByteOrder };

use mio::Evented;

use core::message::Message;
use io_error::*;

pub trait AsyncPipeStub : Sender + Receiver + Handshake + Deref<Target=Evented> {
}

pub trait Sender {
    fn start_send(&mut self, msg: Rc<Message>) -> Result<bool>;
    fn resume_send(&mut self) -> Result<bool>;
    fn has_pending_send(&self) -> bool;
}

pub trait Receiver {
    fn start_recv(&mut self) -> Result<Option<Message>>;
    fn resume_recv(&mut self) -> Result<Option<Message>>;
    fn has_pending_recv(&self) -> bool;
}

pub trait Handshake {
    fn send_handshake(&mut self, pids: (u16, u16)) -> Result<()>;
    fn recv_handshake(&mut self, pids: (u16, u16)) -> Result<()>;
}

pub fn send_and_check_handshake<T:Write>(stream: &mut T, pids: (u16, u16)) -> Result<()> {
    let (proto_id, _) = pids;
    let handshake = create_handshake(proto_id);

    match try!(stream.write(&handshake)) {
        8 => Ok(()),
        _ => Err(would_block_io_error("failed to send handshake"))
    }
}

fn create_handshake(protocol_id: u16) -> [u8; 8] {
    // handshake is Zero, 'S', 'P', Version, Proto[2], Rsvd[2]
    let mut handshake = [0, 83, 80, 0, 0, 0, 0, 0];
    BigEndian::write_u16(&mut handshake[4..6], protocol_id);
    handshake
}

pub fn recv_and_check_handshake<T:Read>(stream: &mut T, pids: (u16, u16)) -> Result<()> {
    let mut handshake = [0u8; 8];

    stream.read(&mut handshake).and_then(|_| check_handshake(pids, &handshake))
}

fn check_handshake(pids: (u16, u16), handshake: &[u8; 8]) -> Result<()> {
    let (_, proto_id) = pids;
    let expected_handshake = create_handshake(proto_id);

    if handshake == &expected_handshake {
        Ok(())
    } else {
        Err(invalid_data_io_error("received bad handshake"))
    }
}