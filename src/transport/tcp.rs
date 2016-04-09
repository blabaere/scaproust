// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::net;
use std::str;
use std::io;

use byteorder::*;

use mio::{ TryRead, TryWrite, Evented };
use mio::tcp;

use transport::{ 
    Transport, 
    Connection, 
    Listener, 
    Conn2, 
    Handshake,
    Sender, 
    Receiver, 
    AsEvented,
    TryWriteBuffer,
    TryReadBuffer,
    send_and_check_handshake,
    recv_and_check_handshake
};

use Message;
use SocketType;
use global;

pub struct Tcp {
    no_delay: bool
}

impl Tcp {
    pub fn new() -> Tcp {
        Tcp { no_delay: false }
    }
}

impl Default for Tcp {
    fn default() -> Tcp {
        Tcp::new()
    }
}

impl Transport for Tcp {

    fn connect(&self, addr: &str) -> io::Result<Box<Connection>> {
        match str::FromStr::from_str(addr) {
            Ok(addr) => self.connect(addr),
            Err(_) => Err(io::Error::new(io::ErrorKind::InvalidInput, addr.to_owned()))
        }
    }

    fn bind(&self, addr: &str) -> io::Result<Box<Listener>> {
        match str::FromStr::from_str(addr) {
            Ok(addr) => self.bind(addr),
            Err(_) => Err(io::Error::new(io::ErrorKind::InvalidInput, addr.to_owned()))
        }
    }

    fn set_nodelay(&mut self, value: bool) {
        self.no_delay = value;
    }
}

impl Tcp {

    fn connect(&self, addr: net::SocketAddr) -> io::Result<Box<Connection>> {
        let tcp_stream = try!(tcp::TcpStream::connect(&addr));
        try!(tcp_stream.set_nodelay(self.no_delay));

        let connection = TcpConnection { stream: tcp_stream };

        Ok(box connection)
    }
    
    fn bind(&self, addr: net::SocketAddr) -> io::Result<Box<Listener>> {
        let tcp_listener = try!(tcp::TcpListener::bind(&addr));
        let listener = TcpListener { 
            listener: tcp_listener,
            no_delay: self.no_delay
        };

        Ok(box listener)
    }
    
}

struct TcpConnection {
    stream: tcp::TcpStream
}

impl Drop for TcpConnection {
    fn drop(&mut self) {
        let _ = self.stream.shutdown(tcp::Shutdown::Both);
    }
}

impl Connection for TcpConnection {
    fn try_read(&mut self, buf: &mut [u8]) -> io::Result<Option<usize>> {
        self.stream.try_read(buf)
    }

    fn try_write(&mut self, buf: &[u8]) -> io::Result<Option<usize>> {
        self.stream.try_write(buf)
    }

    fn as_evented(&self) -> &Evented {
        &self.stream
    }
}

struct TcpListener {
    listener: tcp::TcpListener,
    no_delay: bool
}

impl Listener for TcpListener {

    fn as_evented(&self) -> &Evented {
        &self.listener
    }

    fn accept(&mut self) -> io::Result<Vec<Box<Connection>>> {
        let mut conns: Vec<Box<Connection>> = Vec::new();

        while let Some((s, _)) = try!(self.listener.accept()) {
            try!(s.set_nodelay(self.no_delay));
            conns.push(box TcpConnection { stream: s });
        }

        Ok(conns)
    }
}

struct TcpConn2 {
    stream: tcp::TcpStream,
    send_operation: Option<TcpSendOperation>,
    recv_operation: Option<TcpRecvOperation>
}

impl TcpConn2 {
    fn run_send_operation(&mut self, mut send_operation: TcpSendOperation) -> io::Result<bool> {
        if try!(send_operation.run(&mut self.stream)) {
            Ok(true)
        } else {
            self.send_operation = Some(send_operation);
            Ok(false)
        }
    }

    fn run_recv_operation(&mut self, mut recv_operation: TcpRecvOperation) -> io::Result<Option<Message>> {
        match try!(recv_operation.run(&mut self.stream)) {
            Some(msg) => Ok(Some(msg)),
            None => {
                self.recv_operation = Some(recv_operation);
                Ok(None)
            }
        }
    }
}

impl AsEvented for TcpConn2 {
    fn as_evented(&self) -> &Evented {
        &self.stream
    }
}

impl Handshake for TcpConn2 {
    fn send_handshake(&mut self, socket_type: SocketType) -> io::Result<()> {
        send_and_check_handshake(&mut self.stream, socket_type)
    }

    fn recv_handshake(&mut self, socket_type: SocketType) -> io::Result<()> {
        recv_and_check_handshake(&mut self.stream, socket_type)
    }
}

impl Sender for TcpConn2 {
    fn start_send(&mut self, msg: Rc<Message>) -> io::Result<bool> {
        let send_operation = TcpSendOperation::new(msg);

        self.run_send_operation(send_operation)
    }

    fn resume_send(&mut self) -> io::Result<bool> {
        if let Some(send_operation) = self.send_operation.take() {
            self.run_send_operation(send_operation)
        } else {
            Err(global::other_io_error("Cannot resume send: no pending operation"))
        }
    }

    fn has_pending_send(&self) -> bool {
        self.send_operation.is_some()
    }
}

struct TcpSendOperation {
    step: Option<SendOperationStep>
}

impl TcpSendOperation {
    fn new(msg: Rc<Message>) -> TcpSendOperation {
        TcpSendOperation { 
            step: Some(SendOperationStep::TransportHdr(msg, 0))
        }
    }

    fn run<T:TryWrite>(&mut self, stream: &mut T) -> io::Result<bool> {
        if let Some(step) = self.step.take() {
            self.resume_at(stream, step)
        } else {
            Err(global::other_io_error("Cannot resume already finished send operation"))
        }
    }

    fn resume_at<T:TryWrite>(&mut self, stream: &mut T, step: SendOperationStep) -> io::Result<bool> {
        let mut cur_step = step;

        loop {
            let (passed, next_step) = try!(cur_step.advance(stream));

            if next_step.is_terminal() {
                return Ok(true);
            }
            if !passed {
                self.step = Some(next_step);
                return Ok(false)
            }

            cur_step = next_step;
        }
    }
}

enum SendOperationStep {
    TransportHdr(Rc<Message>, usize),
    ProtocolHdr(Rc<Message>, usize),
    UsrPayload(Rc<Message>, usize),
    Terminal
}

impl SendOperationStep {
    /// Writes one of the buffers composing the message.
    /// Returns whether the buffer was fully sent, and what is the next step.
    fn advance<T:TryWrite>(self, stream: &mut T) -> io::Result<(bool, SendOperationStep)> {
        match self {
            SendOperationStep::TransportHdr(msg, written) => write_transport_hdr(stream, msg, written),
            SendOperationStep::ProtocolHdr(msg, written) => write_protocol_hdr(stream, msg, written),
            SendOperationStep::UsrPayload(msg, written) => write_usr_payload(stream, msg, written),
            SendOperationStep::Terminal => Err(global::other_io_error("Cannot advance terminal step of send operation"))
        }
    }

    fn is_terminal(&self) -> bool {
        match *self {
            SendOperationStep::Terminal => true,
            _ => false,
        }
    }
}

fn write_transport_hdr<T:TryWrite>(stream: &mut T, msg: Rc<Message>, mut written: usize) -> io::Result<(bool, SendOperationStep)> {
    let msg_len = msg.len() as u64;
    let mut buffer = [0u8; 8];

    BigEndian::write_u64(&mut buffer, msg_len);

    let sent = try!(try_write_buffer(stream, &buffer, &mut written));
    if sent {
        Ok((true, SendOperationStep::ProtocolHdr(msg, 0)))
    } else {
        Ok((false, SendOperationStep::TransportHdr(msg, written)))
    }
}

fn write_protocol_hdr<T:TryWrite>(stream: &mut T, msg: Rc<Message>, mut written: usize) -> io::Result<(bool, SendOperationStep)> {
    if msg.get_header().len() == 0 {
        return Ok((true, SendOperationStep::UsrPayload(msg, 0)));
    }

    let sent = try!(try_write_buffer(stream, msg.get_header(), &mut written));
    if sent {
        Ok((true, SendOperationStep::UsrPayload(msg, 0)))
    } else {
        Ok((false, SendOperationStep::ProtocolHdr(msg, written)))
    }
}

fn write_usr_payload<T:TryWrite>(stream: &mut T, msg: Rc<Message>, mut written: usize) -> io::Result<(bool, SendOperationStep)> {
    if msg.get_body().len() == 0 {
        return Ok((true, SendOperationStep::Terminal));
    }

    let sent = try!(try_write_buffer(stream, msg.get_body(), &mut written));
    if sent {
        Ok((true, SendOperationStep::Terminal))
    } else {
        Ok((false, SendOperationStep::UsrPayload(msg, written)))
    }
}

fn try_write_buffer<T:TryWrite>(stream: &mut T, buffer: &[u8], written: &mut usize) -> io::Result<bool> {
    let remaining_buffer = &buffer[*written..];

    *written += try!(stream.try_write_buffer(remaining_buffer));

    Ok(*written == buffer.len())
}

impl Receiver for TcpConn2 {
    fn start_recv(&mut self) -> io::Result<Option<Message>> {
        let recv_operation = TcpRecvOperation::new();

        self.run_recv_operation(recv_operation)
    }

    fn resume_recv(&mut self) -> io::Result<Option<Message>> {
        if let Some(recv_operation) = self.recv_operation.take() {
            self.run_recv_operation(recv_operation)
        } else {
            Err(global::other_io_error("Cannot resume recv: no pending operation"))
        }
    }

    fn has_pending_recv(&self) -> bool {
        self.recv_operation.is_some()
    }
}

enum RecvOperationStep {
    Header([u8; 8], usize),
    Payload(Vec<u8>, usize),
    Terminal(Message)
}

impl RecvOperationStep {
    fn advance<T:TryRead>(self, stream: &mut T) -> io::Result<(bool, RecvOperationStep)> {
        match self {
            RecvOperationStep::Header(buffer, read) => read_header(stream, buffer, read),
            RecvOperationStep::Payload(buffer, read) => read_payload(stream, buffer, read),
            RecvOperationStep::Terminal(_) => Err(global::other_io_error("Cannot advance terminal step of recv operation"))
        }
    }
}

fn read_header<T:TryRead>(stream: &mut T, mut buffer: [u8; 8], mut read: usize) -> io::Result<(bool, RecvOperationStep)> {
    read += try!(stream.try_read_buffer(&mut buffer[read..]));

    if read == 8 {
        let msg_len = BigEndian::read_u64(&buffer);
        let payload = vec![0u8; msg_len as usize];

        Ok((true, RecvOperationStep::Payload(payload, 0)))
    } else {
        Ok((false, RecvOperationStep::Header(buffer, read)))
    }
}

fn read_payload<T:TryRead>(stream: &mut T, mut buffer: Vec<u8>, mut read: usize) -> io::Result<(bool, RecvOperationStep)> {
    read += try!(stream.try_read_buffer(&mut buffer[read..]));

    if read == buffer.capacity() {
        Ok((true, RecvOperationStep::Terminal(Message::with_body(buffer))))
    } else {
        Ok((false, RecvOperationStep::Payload(buffer, read)))
    }
}

struct TcpRecvOperation {
    step: Option<RecvOperationStep>
}

impl TcpRecvOperation {
    fn new() -> TcpRecvOperation {
        TcpRecvOperation {
            step: Some(RecvOperationStep::Header([0; 8], 0))
        }
    }

    fn run<T:TryRead>(&mut self, stream: &mut T) -> io::Result<Option<Message>> {
        if let Some(step) = self.step.take() {
            self.resume_at(stream, step)
        } else {
            Err(global::other_io_error("Cannot resume already finished recv operation"))
        }
    }

    fn resume_at<T:TryRead>(&mut self, stream: &mut T, step: RecvOperationStep) -> io::Result<Option<Message>> {
        let (passed, next_step) = try!(step.advance(stream));

        if !passed {
            self.step = Some(next_step);
            return Ok(None);
        }

        match next_step {
            RecvOperationStep::Terminal(msg) => return Ok(Some(msg)),
            other => self.resume_at(stream, other)
        }
    }
}

impl Conn2 for TcpConn2 {}

#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use std::io;

    use Message;
    use transport::TestTryWrite;
    use super::{ TcpSendOperation, TcpRecvOperation };

    #[test]
    fn send_in_one_run() {
        let header = vec!(1, 4, 3, 2);
        let payload = vec!(65, 66, 67, 69);
        let msg = Message::with_header_and_body(header, payload);
        let mut operation = TcpSendOperation::new(Rc::new(msg));
        let mut stream = TestTryWrite::new();
        let result = operation.run(&mut stream).expect("send should have succeeded");
        let expected_bytes = [0, 0, 0, 0, 0, 0, 0, 8, 1, 4, 3, 2, 65, 66, 67, 69];

        assert!(result);
        assert_eq!(&expected_bytes, stream.get_bytes());
    }

    #[test]
    fn recv_in_one_run() {
        let buffer = vec![0, 0, 0, 0, 0, 0, 0, 8, 1, 4, 3, 2, 65, 66, 67, 69];
        let mut stream = io::Cursor::new(buffer);
        let mut operation = TcpRecvOperation::new();
        let msg = operation.run(&mut stream).
            expect("recv should have succeeded").
            expect("recv should be done");
        let expected_bytes = [1, 4, 3, 2, 65, 66, 67, 69];

        assert_eq!(&expected_bytes, msg.get_body());
    }
}

