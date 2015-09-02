// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::io;
use std::path;
use std::fs;

use mio::{ TryRead, TryWrite, Evented };
use mio::unix;

use super::{ Transport, Connection, Listener };

pub struct Ipc;

impl Transport for Ipc {

    fn connect(&self, addr: &str) -> io::Result<Box<Connection>> {
        self.connect(path::Path::new(addr))
    }

    fn bind(&self, addr: &str) -> io::Result<Box<Listener>> {
        self.bind(path::Path::new(addr))
    }

}

impl Ipc {

    fn connect(&self, addr: &path::Path) -> Result<Box<Connection>, io::Error> {
        let ipc_stream = try!(unix::UnixStream::connect(&addr));
        let connection = IpcConnection { stream: ipc_stream };

        Ok(Box::new(connection))
    }
    
    fn bind(&self, path: &path::Path) -> Result<Box<Listener>, io::Error> {
        if fs::metadata(path).is_ok() {
            let _ = fs::remove_file(path);
        }

        let ipc_listener = try!(unix::UnixListener::bind(path));
        let listener = IpcListener { listener: ipc_listener };

        Ok(Box::new(listener))
    }
    
}

struct IpcConnection {
    stream: unix::UnixStream
}

impl Connection for IpcConnection {
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

struct IpcListener {
    listener: unix::UnixListener
}

impl Listener for IpcListener {

    fn as_evented(&self) -> &Evented {
        &self.listener
    }

    fn accept(&mut self) -> io::Result<Vec<Box<Connection>>> {
        let mut conns: Vec<Box<Connection>> = Vec::new();

        loop {
            match try!(self.listener.accept()) {
                Some(s) => conns.push(Box::new(IpcConnection { stream: s })),
                None    => break
            }
        }

        Ok(conns)
    }
}
