// Copyright (c) 2015-2016 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.


mod stub;
mod acceptor;

use std::io;
use std::fs::OpenOptions;
use std::os::windows::fs::*;
use std::os::windows::io::*;

use mio_named_pipes::NamedPipe;
use winapi;

use self::stub::IpcPipeStub;
use self::acceptor::IpcAcceptor;

use transport::{Transport, Destination};
use transport::pipe::Pipe;
use transport::acceptor::Acceptor;
use transport::async::AsyncPipe;

pub struct Ipc;

impl Transport for Ipc {
    fn connect(&self, dest: &Destination) -> io::Result<Box<Pipe>> {
        let mut options = OpenOptions::new();
        options.read(true).write(true).custom_flags(winapi::FILE_FLAG_OVERLAPPED);
        //let name = format!(r"\\.\{}", dest.addr);
        let name = format!(r"\\.\pipe\my-pipe-{}", dest.addr);
        info!("Creating client pipe: {}", &name);
        let file = try!(options.open(name));
        let named_pipe = unsafe { NamedPipe::from_raw_handle(file.into_raw_handle()) };
        let stub = IpcPipeStub::new_client(named_pipe, dest.recv_max_size);
        let pipe = box AsyncPipe::new(stub, dest.pids);

        Ok(pipe)
    }

    fn bind(&self, dest: &Destination) -> io::Result<Box<Acceptor>> {
        let addr = String::from(dest.addr);
        let acceptor = box IpcAcceptor::new(addr, dest.pids, dest.recv_max_size);

        Ok(acceptor)
    }
}
