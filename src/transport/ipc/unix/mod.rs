// Copyright (c) 2015-2017 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.


mod stub;
mod acceptor;

use std::fs;
use std::io;
use std::path;
use std::os::unix::fs::FileTypeExt;

use mio_uds::{UnixListener, UnixStream};

use self::stub::IpcPipeStub;
use self::acceptor::IpcAcceptor;
use transport::{Transport, Destination};
use transport::pipe::Pipe;
use transport::acceptor::Acceptor;
use transport::async::AsyncPipe;

pub struct Ipc;

impl Transport for Ipc {
    fn connect(&self, dest: &Destination) -> io::Result<Box<dyn Pipe>> {
        let filename = path::Path::new(dest.addr);
        let stream = UnixStream::connect(filename)?;
        let stub = IpcPipeStub::new(stream, dest.recv_max_size);
        let pipe = AsyncPipe::new(stub, dest.pids);

        Ok(Box::new(pipe))
    }

    fn bind(&self, dest: &Destination) -> io::Result<Box<dyn Acceptor>> {
        let filename = path::Path::new(dest.addr);

        match fs::metadata(filename).map(|meta| meta.file_type().is_socket()) {
            Ok(true)  => fs::remove_file(filename)?,
            _ => (),
        }

        let listener = UnixListener::bind(filename)?;
        let acceptor = IpcAcceptor::new(listener, dest.pids, dest.recv_max_size);

        Ok(Box::new(acceptor))
    }
}
