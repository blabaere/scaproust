// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::net;
use std::str;
use std::io;
use super::{ Transport, Connection, Listener };
use mio::{ TryRead, TryWrite, Evented };
use mio::tcp;

pub struct Tcp;

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

}

impl Tcp {

    fn connect(&self, addr: net::SocketAddr) -> Result<Box<Connection>, io::Error> {
        let tcp_stream = try!(tcp::TcpStream::connect(&addr));
        let connection = TcpConnection { stream: tcp_stream };

        Ok(Box::new(connection))
    }
    
    fn bind(&self, addr: net::SocketAddr) -> Result<Box<Listener>, io::Error> {
        let tcp_listener = try!(tcp::TcpListener::bind(&addr));
        let listener = TcpListener { listener: tcp_listener };

        Ok(Box::new(listener))
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
    fn try_read(&mut self, buf: &mut [u8]) -> Result<Option<usize>, io::Error> {
        self.stream.try_read(buf)
    }

    fn try_write(&mut self, buf: &[u8]) -> Result<Option<usize>, io::Error> {
        self.stream.try_write(buf)
    }

    fn as_evented(&self) -> &Evented {
        &self.stream
    }
}

struct TcpListener {
    listener: tcp::TcpListener
}

impl Listener for TcpListener {

    fn as_evented(&self) -> &Evented {
        &self.listener
    }

    fn accept(&mut self) -> io::Result<Vec<Box<Connection>>> {
        let mut conns: Vec<Box<Connection>> = Vec::new();

        loop {
            match try!(self.listener.accept()) {
                Some((s, _)) => conns.push(Box::new(TcpConnection { stream: s })),
                None    => break
            }
        }

        Ok(conns)
    }
}
