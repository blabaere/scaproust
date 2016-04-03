#[macro_use] extern crate log;
extern crate env_logger;
extern crate scaproust;

use std::io;

use scaproust::{Session, SocketType, Socket};

fn handle_comand(cmd: &str, socket: &mut Socket, count: usize) {
    println!("User command: {:?}", cmd);
    let big: usize = 3/* * 1024 * 1024*/; // this to force a partial write
    let mut msg = Vec::with_capacity(big);
    for x in count..count+big {
        let index = (x % 26) as u8;
        msg.push(65 + index);
    }
    match socket.send(msg) {
        Ok(_) => info!("message sent !"),
        Err(e) => error!("message NOT sent: {} !", e)
    }
}

fn main() {

    env_logger::init().unwrap();
    info!("Logging initialized.");

    let session = Session::new().unwrap();
    let mut socket = session.create_socket(SocketType::Push).unwrap();

    socket.connect("tcp://127.0.0.1:5454").unwrap();
    //socket.connect("tcp://127.0.0.1:5455").unwrap();
    //socket.bind("tcp://127.0.0.1:5456").unwrap();

    let mut input = String::new();
    let mut count = 0;
    loop {
        match io::stdin().read_line(&mut input) {
            Ok(0) => return,
            Ok(_) => handle_comand(&input ,&mut socket, count),
            Err(error) => println!("error: {}", error),
        };
        input.clear();
        count = count + 1;
    }
}
