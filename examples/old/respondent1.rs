#[macro_use] extern crate log;
extern crate env_logger;
extern crate scaproust;

use std::io::*;

use scaproust::{Session, SocketType};

fn main() {

    env_logger::init().unwrap();
    info!("Logging initialized.");

    let session = Session::new().unwrap();
    let mut socket = session.create_socket(SocketType::Respondent).unwrap();

    socket.connect("tcp://127.0.0.1:5459").unwrap();

    let mut input = String::new();

    match socket.recv_msg() {
        Ok(msg) => info!("survey received '{:?}' !", msg.get_body()),
        Err(e) => error!("survey NOT received: {} !", e)
    }
    /*stdout().write(b"Hit 'Enter' to send the vote\n").unwrap();
    stdin().read_line(&mut input).unwrap();*/
    match socket.send(vec!(99, 66, 88)) {
        Ok(_) => info!("vote sent !"),
        Err(e) => error!("vote NOT sent: {} !", e)
    }

    stdout().write(b"Hit 'Enter' to exit ...\n").unwrap();
    stdin().read_line(&mut input).unwrap();

}
