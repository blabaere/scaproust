#[macro_use] extern crate log;
extern crate env_logger;
extern crate scaproust;

use std::io;

use scaproust::{Session, SocketType};

fn handle_comand(cmd: &str) {
	println!("User command: {:?}", cmd);
}

fn main() {

    env_logger::init().unwrap();
    info!("Logging initialized.");

    let session = Session::new().unwrap();
    let socket = session.create_socket(SocketType::Push).unwrap();

    socket.connect("tcp://127.0.0.1:5454");

    let mut input = String::new();
    loop {
		match io::stdin().read_line(&mut input) {
		    Ok(0) => return,
		    Ok(_) => handle_comand(&input),
		    Err(error) => println!("error: {}", error),
		};
		input.clear();
    }
}
