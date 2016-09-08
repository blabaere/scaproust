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

use scaproust::*;

fn create_session() -> Session {
    SessionBuilder::new().
        with("tcp", Tcp).
        with("ipc", Ipc).
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

fn usage(program: &str) -> ! {
    let _ = writeln!(stderr(), "Usage: {} <NODE_NAME> <URL> <URL> ...", program);
    std::process::exit(1)
}

fn node(args: Vec<&str>) {
    let mut session = create_session();
    let mut socket = session.create_socket::<Bus>().expect("Failed to create socket !");

    socket.bind(args[2]).expect("Failed to bind socket !");
    sleep_ms(100);
    if args.len() > 2 {
        for x in 3..args.len() {
            socket.connect(args[x]).expect("Failed to connect socket !");
        }
    }
    sleep_ms(1000);
    socket.set_recv_timeout(make_timeout(100)).expect("Failed to set recv timeout !");

    let name = args[1];
    let buffer = From::from(name.as_bytes());
    println!("{}: SENDING '{}' ONTO BUS", name, name);
    socket.send(buffer).expect("Send failed !");

    loop {
        match socket.recv() {
            Ok(buf) => {
                let msg = std::str::from_utf8(&buf).expect("Failed to parse msg !");
                println!("{}: RECEIVED '{}' FROM BUS", name, msg);
            },
            Err(e) => {
                if e.kind() == ErrorKind::TimedOut {
                    continue;
                } else {
                    panic!("recv failed !");
                }
            }
        };
    }
}

fn main() {
    env_logger::init().unwrap();

    let os_args: Vec<_> = std::env::args().collect();
    let args: Vec<&str> = os_args.iter().map(|x| x.as_ref()).collect();
    let program = args[0];

    if args.len() < 3 {
        usage(program);
    }

    node(args)
}