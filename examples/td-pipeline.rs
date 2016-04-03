#[macro_use] extern crate log;
extern crate env_logger;
extern crate scaproust;

use std::io::*;
use std::time;
use std::thread;

use scaproust::{Session, SocketType};

const NODE0: &'static str = "node0";
const NODE1: &'static str = "node1";

fn sleep_ms(ms: u64) {
    thread::sleep(time::Duration::from_millis(ms));
}

fn node0(url: &str) {
    let session = Session::new().expect("Failed to create session !");
    let mut socket = session.create_socket(SocketType::Pull).expect("Failed to create socket !");

    socket.bind(url).expect("Failed to bind socket !");

    loop {
        let buffer = socket.recv().expect("Recv failed !");
        let msg = std::str::from_utf8(&buffer).expect("Failed to parse msg !");

        println!("NODE0: RECEIVED \"{}\"", msg);
   }
}

fn node1(url: &str, msg: &str) {
    let session = Session::new().expect("Failed to create session !");
    let mut socket = session.create_socket(SocketType::Push).expect("Failed to create socket !");
    let buffer = From::from(msg.as_bytes());

    socket.connect(url).expect("Failed to connect socket !");
    println!("NODE1: SENDING \"{}\"", msg);
    socket.send(buffer).expect("Send failed !");
    sleep_ms(50); // TODO remove this when linger is implemented ?
}

fn usage(program: &str) -> ! {
    let _ = writeln!(stderr(), "Usage: {} {}|{} <URL> <ARG> ...", program, NODE0, NODE1);
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
        NODE0 if args.len() == 3 => node0(args[2]),
        NODE1 if args.len() == 4 => node1(args[2], args[3]),
        _ => usage(program)
    }
}