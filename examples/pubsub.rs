// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
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
use std::time;
use std::thread;
use std::fmt;

use scaproust::*;

const SERVER: &'static str = "server";
const CLIENT: &'static str = "client";

#[cfg(not(windows))]
fn create_session() -> Session {
    SessionBuilder::new().
        with("tcp", Tcp).
        //with("ipc", Ipc).
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

fn server(url: &str) {
    let mut session = create_session();
    let mut socket = session.create_socket::<Pub>().expect("Failed to create socket !");

    socket.bind(url).expect("Failed to bind socket !");
    loop {
        let date = "1970/01/01 00:00:00.666";
        println!("SERVER: PUBLISHING DATE {:?}", date);
        let reply = fmt::format(format_args!("{:?}!", date));
        let buffer = From::from(reply.as_bytes());

        socket.send(buffer).expect("Send failed !");

        sleep_ms(1000);
    }
}

fn client(url: &str, name: &str) {
    let mut session = create_session();
    let mut socket = session.create_socket::<Sub>().expect("Failed to create socket !");

    socket.set_option(ConfigOption::Subscribe("".to_string())).expect("Failed to subscribe !");
    socket.connect(url).expect("Failed to connect socket !");

    loop {
        let buffer = socket.recv().expect("Recv failed !");
        let msg = std::str::from_utf8(&buffer).expect("Failed to parse msg !");

        println!("CLIENT ({}): RECEIVED \"{}\"", name, msg);
    }
}

fn usage(program: &str) -> ! {
    let _ = writeln!(stderr(), "Usage: {} {}|{} <URL> <ARG> ...", program, SERVER, CLIENT);
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
        SERVER if args.len() == 3 => server(args[2]),
        CLIENT if args.len() == 4 => client(args[2], args[3]),
        _ => usage(program)
    }
}