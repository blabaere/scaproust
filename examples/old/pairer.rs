#[macro_use] extern crate log;
extern crate env_logger;
extern crate scaproust;

use std::io;

use scaproust::{Session, SocketType, Socket};

fn handle_comand(cmd: &str, socket: &mut Socket) {
    println!("User command: {:?}", cmd);
    /*let big: usize = 128/* * 1024 * 1024*/; // this to force a partial write
    let mut msg = Vec::with_capacity(big);
    for x in 0..big {
        let index = (x % 26) as u8;
        msg.push(65 + index);
    }
    match socket.send(msg) {
        Ok(_) => info!("message sent !"),
        Err(e) => error!("message NOT sent: {} !", e)
    }*/
    match socket.recv_msg() {
        Ok(msg) => info!("message received '{:?}'!", msg.len()),
        Err(e) => error!("message NOT received: {} !", e)
    }
}

fn main() {

    env_logger::init().unwrap();
    info!("Logging initialized.");

    let session = Session::new().expect("session could not be created !");
    let mut socket = session.create_socket(SocketType::Pair).expect("socket could not be created !");

    socket.connect("tcp://127.0.0.1:5459").expect("socket could not be connected/bound !");

    let mut input = String::new();
    loop {
        match io::stdin().read_line(&mut input) {
            Ok(0) => return,
            Ok(_) => handle_comand(&input ,&mut socket),
            Err(error) => println!("error: {}", error),
        };
        input.clear();
    }
}
