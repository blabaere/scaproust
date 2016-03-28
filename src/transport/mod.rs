// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::io;
use mio;

pub mod tcp;
#[cfg(not(windows))]
pub mod ipc;

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
    fn try_read(&mut self, buf: &mut [u8]) -> Result<Option<usize>, io::Error>;
    fn try_write(&mut self, buf: &[u8]) -> Result<Option<usize>, io::Error>;
}

pub trait Listener {
    fn as_evented(&self) -> &mio::Evented;
    fn accept(&mut self) -> io::Result<Vec<Box<Connection>>>;
}

pub fn create_transport(name: &str) -> io::Result<Box<Transport>> {
    match name {
        "tcp" => Ok(box tcp::Tcp::new()),
        #[cfg(not(windows))]
        "ipc" => Ok(box ipc::Ipc),
        _     => Err(io::Error::new(io::ErrorKind::InvalidData, format!("'{}' is not a supported protocol (tcp or ipc)", name)))
    }
    
}
