// Copyright (c) 2015-2017 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.
//
// This file was adapted from the nanomsg C example written by Tim Dysinger.
// See http://tim.dysinger.net/posts/2013-09-16-getting-started-with-nanomsg.html
// and https://github.com/dysinger/nanomsg-examples.

extern crate log;
extern crate env_logger;
extern crate scaproust;

use std::io::*;
use std::time;
use std::thread;

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

fn sleep_ms(ms: u64) {
    thread::sleep(time::Duration::from_millis(ms));
}

fn node0(url: &str) {
    let mut session = create_session();
    let mut socket = session.create_socket::<Rep>().expect("Failed to create socket !");

    socket.bind(url).expect("Failed to bind socket !");

    loop {
        let buffer = socket.recv().expect("Recv request failed !");
        let request = std::str::from_utf8(&buffer).expect("Failed to parse request msg !");

        println!("NODE0: RECEIVED {:?}", request);
        let response: String = request.chars().rev().collect();
        println!("NODE0: SENDING: {:?}", response);
        socket.send(response.into_bytes()).expect("Send reply failed!");
   }
}

fn node1(url: &str) {
    let mut session = create_session();
    let mut socket = session.create_socket::<Req>().expect("Failed to create socket !");
    socket.connect(url).expect("Failed to connect socket !");

    let stdin = BufReader::new(stdin());

    for line in stdin.lines() {
        let line = line.expect("Failed to read line");
        println!("NODE1: SENDING REQUEST {:?}", line);
        socket.send(line.into_bytes()).expect("Send request failed !");
        let buffer = socket.recv().expect("Recv reply failed !");
        let reply = std::str::from_utf8(&buffer).expect("Failed to parse reply msg !");
        println!("NODE1: RECEIVED RESPONSE {:?}", reply);
    }

    sleep_ms(50); // TODO remove this when linger is implemented ?
}

fn usage(program: &str) -> ! {
    let _ = writeln!(stderr(), "Usage: {} {}|{} <URL> ...", program, NODE0, NODE1);
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
        NODE1 if args.len() == 3 => node1(args[2]),
        _ => usage(program)
    }
}
