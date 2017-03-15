// Copyright (c) 2015-2017 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

mod stub;
mod send;
mod recv;
mod acceptor;

use std::str::FromStr;
use std::io;
use std::net;

use mio::tcp::{TcpListener, TcpStream};

use self::stub::TcpPipeStub;
use self::acceptor::TcpAcceptor;
use transport::{Transport, Destination};
use transport::pipe::Pipe;
use transport::acceptor::Acceptor;
use transport::async::AsyncPipe;
use io_error::*;

pub struct Tcp;

impl Tcp {
    fn connect(&self, addr: &net::SocketAddr, dest: &Destination) -> io::Result<Box<Pipe>> {
        let stream = try!(TcpStream::connect(addr));
        try!(stream.set_nodelay(dest.tcp_no_delay));
        let stub = TcpPipeStub::new(stream, dest.recv_max_size);
        let pipe = AsyncPipe::new(stub, dest.pids);

        Ok(Box::new(pipe))
    }
    fn bind(&self, addr: &net::SocketAddr, dest: &Destination) -> io::Result<Box<Acceptor>> {
        let listener = try!(TcpListener::bind(addr));
        let acceptor = TcpAcceptor::new(listener, dest);

        Ok(Box::new(acceptor))
    }
}

impl Transport for Tcp {
    fn connect(&self, dest: &Destination) -> io::Result<Box<Pipe>> {
        match net::SocketAddr::from_str(dest.addr) {
            Ok(addr) => self.connect(&addr, dest),
            Err(_) => Err(invalid_input_io_error(dest.addr))
        }
    }

    fn bind(&self, dest: &Destination) -> io::Result<Box<Acceptor>> {
        match net::SocketAddr::from_str(dest.addr) {
            Ok(addr) => self.bind(&addr, dest),
            Err(_) => Err(invalid_input_io_error(dest.addr))
        }
    }
}
