// Copyright (c) 2015-2016 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.
//
// This file was adapted from the nanomsg C example written by Tim Dysinger.
// See http://tim.dysinger.net/posts/2013-09-16-getting-started-with-nanomsg.html
// and https://github.com/dysinger/nanomsg-examples.

#[macro_use] extern crate log;
extern crate env_logger;
extern crate scaproust;

use std::io::*;

use scaproust::*;

const NODE0: &'static str = "node0";
const NODE1: &'static str = "node1";

#[cfg(not(windows))]
fn create_session() -> Session {
    SessionBuilder::new().
        with("tcp", Tcp).
        with("ipc", Ipc).
        build().expect("Failed to create session !")
}

#[cfg(windows)]
fn create_session() -> Session {
    SessionBuilder::new().
        with("tcp", Tcp).
        build().expect("Failed to create session !")
}

fn node0(url: &str) {
    let mut session = create_session();
    let mut socket = session.create_socket::<Pull>().expect("Failed to create socket !");
    let _ = socket.bind(url).expect("Failed to bind socket !");

    loop {
        let buffer = socket.recv().expect("Recv failed !");
        let msg = std::str::from_utf8(&buffer).expect("Failed to parse msg !");

        println!("NODE0: RECEIVED \"{}\"", msg);
   }
}

fn node1(url: &str, msg: &str) {
    let mut session = create_session();
    let mut socket = session.create_socket::<Push>().expect("Failed to create socket !");
    let _ = socket.connect(url).expect("Failed to connect socket !");
    let buffer = From::from(msg.as_bytes());

    println!("NODE1: SENDING \"{}\"", msg);
    socket.send(buffer).expect("Send failed !");
}

fn usage(program: &str) -> ! {
    let _ = writeln!(stderr(), "Usage: {} {}|{} <URL> <ARG> ...", program, NODE0, NODE1);
    std::process::exit(1)
}

fn main() {
    env_logger::init().unwrap();

    let os_args: Vec<_> = std::env::args().collect();
    let args: Vec<&str> = os_args.iter().map(|x| x.as_ref()).collect();
    let program = args[0];

    if args.len() < 2 {
        usage(program);
    }

    match args[1] {
        NODE0 if args.len() == 3 => node0(args[2]),
        NODE1 if args.len() == 4 => node1(args[2], args[3]),
        _ => usage(program)
    }
}
