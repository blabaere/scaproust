#[macro_use] extern crate log;
extern crate env_logger;
extern crate scaproust;

use std::io;

use scaproust::{Session, SocketType, Socket};

fn handle_comand(cmd: &str, socket: &mut Socket) {
	println!("User command: {:?}", cmd);
    match socket.send(vec!(65, 66, 67)) {
        Ok(_) => info!("request sent !"),
        Err(e) => error!("request NOT sent: {} !", e)
    }

    match socket.recv_msg() {
        Ok(msg) => info!("reply received '{:?}' !", msg.get_body()),
        Err(e) => error!("reply NOT received: {} !", e)
    }
}

fn main() {

    env_logger::init().unwrap();
    info!("Logging initialized.");

    let session = Session::new().unwrap();
    let mut socket = session.create_socket(SocketType::Req).unwrap();

    socket.connect("tcp://127.0.0.1:5458").unwrap();

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
