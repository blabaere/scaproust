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
    let _ = writeln!(stderr(), "Usage: {} <bind-to> <msg-size> <msg-count>", program);
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
    let msg_count = usize::from_str(args[3]).expect("Failed to parse msg-count");

    let mut session = create_session();
    let mut socket = session.create_socket::<Pair>().expect("Failed to create socket !");

    socket.bind(url).expect("Failed to bind socket !");

    thread::sleep(Duration::from_millis(250));

    let first_msg = socket.recv().unwrap();
    assert_eq!(0, first_msg.len());

    let start = Instant::now();
    for _ in 0..msg_count {
        let msg = socket.recv().unwrap();
        assert_eq!(msg_size, msg.len());
    }

    let elapsed  = start.elapsed();
    let seconds = elapsed.as_secs() as f64;
    let nanos = elapsed.subsec_nanos() as f64;
    let elapsed_seconds = seconds + nanos / 1_000_000_000f64;
    let msg_per_sec = msg_count as f64 / elapsed_seconds;
    let mb_per_sec = (msg_per_sec * msg_size as f64 * 8f64) / 1_000_000f64;

    println!("message size: {} [B]", msg_size);
    println!("message count: {}", msg_count);
    println!("throughput: {:0.0} [msg/s]", msg_per_sec);
    println!("throughput: {:0.3} [Mb/s]", mb_per_sec);
}