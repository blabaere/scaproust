// Copyright (c) 2015-2016 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

extern crate log;
extern crate env_logger;
extern crate scaproust;

use std::io::*;
use std::time::*;
use std::thread;
use std::str::FromStr;

use scaproust::*;
fn create_session() -> Session {
    SessionBuilder::new().
        with("tcp", Tcp).
        build().expect("Failed to create session !")
}

fn usage(program: &str) -> ! {
    let _ = writeln!(stderr(), "Usage: {} <bind-to> <msg-size> <roundtrips>", program);
    std::process::exit(1)
}

fn main() {
    env_logger::init().unwrap();

    let os_args: Vec<_> = std::env::args().collect();
    let args: Vec<&str> = os_args.iter().map(|x| x.as_ref()).collect();
    let program = args[0];

    if args.len() != 4 {
        usage(program);
    }

    let url = &args[1];
    let msg_size = usize::from_str(args[2]).expect("Failed to parse msg-size");
    let roundtrips = usize::from_str(args[3]).expect("Failed to parse roundtrips");

    let mut session = create_session();
    let mut socket = session.create_socket::<Pair>().expect("Failed to create socket !");

    socket.set_tcp_nodelay(true).expect("Failed to set tcp nodelay !");
    socket.bind(url).expect("Failed to bind socket !");

    thread::sleep(Duration::from_millis(250));

    for _ in 0..roundtrips {
        let msg = socket.recv_msg().unwrap();
        assert_eq!(msg_size, msg.len());
        socket.send_msg(msg).unwrap();
    }

    thread::sleep(Duration::from_millis(1000));
}
