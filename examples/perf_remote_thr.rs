// Copyright (c) 2015-2016 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

#[macro_use] extern crate log;
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
    let _ = writeln!(stderr(), "Usage: {} <connect-to> <msg-size> <msg-count>", program);
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
    let msg_size = usize::from_str(&args[2]).expect("Failed to parse msg-size");
    let msg_count = usize::from_str(&args[3]).expect("Failed to parse msg-count");

    let mut session = create_session();
    let mut socket = session.create_socket::<Pair>().expect("Failed to create socket !");

    socket.connect(url).expect("Failed to connect socket !");

    thread::sleep(Duration::from_millis(250));

    socket.send(Vec::new()).unwrap();

    let start = Instant::now();
    for _ in 0..msg_count {
        let buffer = vec![6; msg_size];

        socket.send(buffer).unwrap();
    }
    let elapsed  = start.elapsed();
    let seconds = elapsed.as_secs() as f64;
    let nanos = elapsed.subsec_nanos() as f64;
    let elapsed_seconds = seconds + nanos / 1_000_000_000f64;

    info!("It took {:?} secs to send {} messages of {} [B]", elapsed_seconds, msg_count, msg_size);

    thread::sleep(Duration::from_millis(1000));
}