// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::io;

use byteorder::{ BigEndian, ByteOrder };

use mio;

use Message;
use SocketType;
use global;

pub mod tcp;
#[cfg(not(windows))]
pub mod ipc;

pub fn create_transport(name: &str) -> io::Result<Box<Transport>> {
    match name {
        "tcp" => Ok(box tcp::Tcp::new()),
        #[cfg(not(windows))]
        "ipc" => Ok(box ipc::Ipc),
        _     => Err(io::Error::new(io::ErrorKind::InvalidData, format!("'{}' is not a supported protocol (tcp or ipc)", name)))
    }
    
}

// represents the transport media 
pub trait Transport {
    fn connect(&self, addr: &str) -> io::Result<Box<Connection>>;
    fn bind(&self, addr: &str) -> io::Result<Box<Listener>>;
    fn set_nodelay(&mut self, _: bool) {}
}

// represents a connection in a given media
// only needs to expose mio compatible features:
// - transfert bytes in non-blocking manner
// - being registrable into the event loop
pub trait Connection {
    fn as_evented(&self) -> &mio::Evented;
    fn try_read(&mut self, buf: &mut [u8]) -> io::Result<Option<usize>>;
    fn try_write(&mut self, buf: &[u8]) -> io::Result<Option<usize>>;
}

pub trait Sender {
    fn start_send(&mut self, msg: Rc<Message>) -> io::Result<bool>;
    fn resume_send(&mut self) -> io::Result<bool>;
    fn has_pending_send(&self) -> bool;
}

pub trait Receiver {
    fn start_recv(&mut self) -> io::Result<Option<Message>>;
    fn resume_recv(&mut self) -> io::Result<Option<Message>>;
    fn has_pending_recv(&self) -> bool;
}

pub trait AsEvented {
    fn as_evented(&self) -> &mio::Evented; // can't get AsRef to compile
}

pub trait Handshake {
    fn send_handshake(&mut self, socket_type: SocketType) -> io::Result<()>;
    fn recv_handshake(&mut self, socket_type: SocketType) -> io::Result<()>;
}

pub trait Conn2 : AsEvented+Handshake+Sender+Receiver {
}

pub trait Listener {
    fn as_evented(&self) -> &mio::Evented;
    fn accept(&mut self) -> io::Result<Vec<Box<Connection>>>;
}

pub trait TryWriteBuffer {
    fn try_write_buffer(&mut self, buffer: &[u8]) -> io::Result<usize>;
}

impl<T:mio::TryWrite> TryWriteBuffer for T {
    fn try_write_buffer(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let written = match try!(self.try_write(buffer)) {
            Some(x) => x,
            None    => 0
        };
        Ok(written)
    }
}

pub trait TryReadBuffer {
    fn try_read_buffer(&mut self, buffer: &mut [u8]) -> io::Result<usize>;
}

impl<T:mio::TryRead> TryReadBuffer for T {
    fn try_read_buffer(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let read = match try!(self.try_read(buffer)) {
            Some(x) => x,
            None    => 0
        };
        Ok(read)
    }
}

fn send_and_check_handshake<T:mio::TryWrite>(stream: &mut T, socket_type: SocketType) -> io::Result<()> {
    let handshake = create_handshake(socket_type);

    match try!(stream.try_write_buffer(&handshake)) {
        8 => Ok(()),
        _ => Err(global::would_block_io_error("failed to send handshake"))
    }
}

fn create_handshake(socket_type: SocketType) -> [u8; 8] {
    // handshake is Zero, 'S', 'P', Version, Proto[2], Rsvd[2]
    let protocol_id = socket_type.id();
    let mut handshake = [0, 83, 80, 0, 0, 0, 0, 0];
    BigEndian::write_u16(&mut handshake[4..6], protocol_id);
    handshake
}

fn recv_and_check_handshake<T:mio::TryRead>(stream: &mut T, socket_type: SocketType) -> io::Result<()> {
    let mut handshake = [0u8; 8];

    stream.try_read_buffer(&mut handshake).and_then(|_| check_handshake(socket_type, &handshake))
}

fn check_handshake(socket_type: SocketType, handshake: &[u8; 8]) -> io::Result<()> {
    let expected_handshake = create_handshake(socket_type.peer());

    if handshake == &expected_handshake {
        Ok(())
    } else {
        Err(global::invalid_data_io_error("received bad handshake"))
    }
}

#[cfg(test)]
struct TestTryWrite {
    buffer: Vec<u8>
}

#[cfg(test)]
impl TestTryWrite {
    fn new() -> TestTryWrite {
        TestTryWrite {
            buffer: Vec::new()
        }
    }

    fn get_bytes<'a>(&'a self) -> &'a [u8] {
        &self.buffer
    }
}

#[cfg(test)]
impl io::Write for TestTryWrite {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
