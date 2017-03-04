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

fn duration_ms(ms: u64) -> time::Duration {
    time::Duration::from_millis(ms)
}

fn sleep_ms(ms: u64) {
    thread::sleep(duration_ms(ms));
}

fn make_timeout(ms: u64) -> Option<time::Duration> {
    Some(duration_ms(ms))
}

fn recv_name(socket: &mut Socket, name: &str) {
    if let Ok(buffer) = socket.recv() {
        let msg = std::str::from_utf8(&buffer).expect("Failed to parse msg !");
        println!("{}: RECEIVED \"{}\"", name, msg);
    }
}

fn send_name(socket: &mut Socket, name: &str) {
    println!("{}: SENDING \"{}\"", name, name);
    let buffer = From::from(name.as_bytes());
    socket.send(buffer).expect("Send failed !");
}

fn send_recv(mut socket: Socket, name: &str) -> ! {
    socket.set_recv_timeout(make_timeout(100)).expect("Failed to set recv timeout !");
    loop {
        recv_name(&mut socket, name);
        sleep_ms(1000);
        send_name(&mut socket, name);
    }
}

fn node0(url: &str) {
    let mut session = create_session();
    let mut socket = session.create_socket::<Pair>().expect("Failed to create socket !");

    socket.bind(url).expect("Failed to bind socket !");
    send_recv(socket, NODE0);
}

fn node1(url: &str) {
    let mut session = create_session();
    let mut socket = session.create_socket::<Pair>().expect("Failed to create socket !");

    socket.connect(url).expect("Failed to connect socket !");
    send_recv(socket, NODE1);
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
