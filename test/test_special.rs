extern crate mio;
#[macro_use] extern crate log;
extern crate env_logger;
extern crate bytes;
extern crate slab;

use mio::*;
use mio::tcp::*;
use std::io;
use std::net::SocketAddr;
use std::str::FromStr;

fn localhost() -> SocketAddr {
    let s = format!("127.0.0.1:{}", 5459);
    FromStr::from_str(&s).unwrap()
}

const SERVER: Token = Token(0);
const CLIENT: Token = Token(1);

struct EchoConn {
    sock: TcpStream,
    token: Option<Token>,
}

type Slab<T> = slab::Slab<T, Token>;

impl EchoConn {
    fn new(sock: TcpStream) -> EchoConn {
        EchoConn {
            sock: sock,
            token: None,
        }
    }

    fn writable(&mut self, _: &mut EventLoop<Echo>) -> io::Result<()> {

        let mut buffer = [0, 6, 6, 6, 0, 10, 20, 0, 65, 66, 67];
        self.write_a_small_msg(&buffer);

        buffer = [0, 6, 6, 6, 0, 10, 20, 0, 99, 66, 88];
        self.write_a_small_msg(&buffer);

        Ok(())
    }

    fn write_a_small_msg(&mut self, buffer: &[u8]) {
        match self.sock.try_write(buffer) {
            Ok(None) => {
                info!("client flushing buf; WOULDBLOCK");
            }
            Ok(Some(r)) => {
                info!("CONN : we wrote {} bytes!", r);
            }
            Err(e) => info!("not implemented; client err={:?}", e),
        }
    }

    fn readable(&mut self, _: &mut EventLoop<Echo>) -> io::Result<()> {
        Ok(())
    }
}

struct EchoServer {
    sock: TcpListener,
    conns: Slab<EchoConn>
}

impl EchoServer {
    fn accept(&mut self, event_loop: &mut EventLoop<Echo>) -> io::Result<()> {
        info!("server accepting socket");

        let sock = self.sock.accept().unwrap().unwrap().0;
        let conn = EchoConn::new(sock,);
        let tok = self.conns.insert(conn)
            .ok().expect("could not add connection to slab");

        // Register the connection
        self.conns[tok].token = Some(tok);
        event_loop.register(&self.conns[tok].sock, tok, EventSet::readable() | EventSet::writable(),
                                PollOpt::edge())
            .ok().expect("could not register socket with event loop");

        Ok(())
    }

    fn conn_readable(&mut self, event_loop: &mut EventLoop<Echo>,
                     tok: Token) -> io::Result<()> {
        info!("server conn readable; tok={:?}", tok);
        self.conn(tok).readable(event_loop)
    }

    fn conn_writable(&mut self, event_loop: &mut EventLoop<Echo>,
                     tok: Token) -> io::Result<()> {
        info!("server conn writable; tok={:?}", tok);
        self.conn(tok).writable(event_loop)
    }

    fn conn<'a>(&'a mut self, tok: Token) -> &'a mut EchoConn {
        &mut self.conns[tok]
    }
}

struct EchoClient {
    sock: TcpStream,
    token: Token,
    received_count: usize
}


impl EchoClient {
    fn new(sock: TcpStream, tok: Token) -> EchoClient {

        EchoClient {
            sock: sock,
            token: tok,
            received_count: 0
        }
    }

    fn readable(&mut self, event_loop: &mut EventLoop<Echo>) -> io::Result<()> {
        info!("client socket readable");

        let mut buffer = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

        match self.sock.try_read(&mut buffer) {
            Ok(None) => {
                info!("CLIENT : spurious read wakeup");
            }
            Ok(Some(r)) => {
                info!("CLIENT : We read {} bytes!", r);
                if r == 11 {
                    self.received_count += 1;
                } else {
                    panic!("client should have been able to read 11 bytes");
                }
            }
            Err(e) => {
                panic!("not implemented; client err={:?}", e);
            }
        };

        if self.received_count == 2 {
            event_loop.shutdown(); 
            Ok(())
        } else {
            self.reregister(event_loop)
        }

    }

    fn writable(&mut self, _: &mut EventLoop<Echo>) -> io::Result<()> {
        info!("client socket writable");
        Ok(())
    }

    fn reregister(&mut self, event_loop: &mut EventLoop<Echo>) -> io::Result<()> {
        let interest = EventSet::readable() | EventSet::writable();
        let poll_opt = PollOpt::edge();

        event_loop.reregister(&self.sock, self.token, interest, poll_opt)
    }
}

struct Echo {
    server: EchoServer,
    client: EchoClient,
}

impl Echo {
    fn new(srv: TcpListener, client: TcpStream) -> Echo {
        Echo {
            server: EchoServer {
                sock: srv,
                conns: Slab::new_starting_at(Token(2), 128)
            },
            client: EchoClient::new(client, CLIENT)
        }
    }
}

impl Handler for Echo {
    type Timeout = usize;
    type Message = ();

    fn ready(&mut self, event_loop: &mut EventLoop<Echo>, token: Token,
             events: EventSet) {
        info!("ready {:?} {:?}", token, events);
        if events.is_readable() {
            match token {
                SERVER => self.server.accept(event_loop).unwrap(),
                CLIENT => self.client.readable(event_loop).unwrap(),
                i => self.server.conn_readable(event_loop, i).unwrap()
            }
        }

        if events.is_writable() {
            match token {
                SERVER => panic!("received writable for token 0"),
                CLIENT => self.client.writable(event_loop).unwrap(),
                _ => self.server.conn_writable(event_loop, token).unwrap()
            };
        }
    }
}

#[test]
fn main() {
    let _ = env_logger::init();
    info!("Logging initialized");
    info!("Starting TEST_ECHO_SERVER");
    let mut event_loop = EventLoop::new().unwrap();

    let addr = localhost();
    let srv = TcpListener::bind(&addr).unwrap();

    info!("listen for connections");
    event_loop.register(&srv, SERVER, EventSet::readable(),
                            PollOpt::edge() | PollOpt::oneshot()).unwrap();

    let sock = TcpStream::connect(&addr).unwrap();

    // Connect to the server
    event_loop.register(&sock, CLIENT, 
        EventSet::readable() | EventSet::writable(),
        PollOpt::edge()).unwrap();

    // Start the event loop
    event_loop.run(&mut Echo::new(srv, sock)).unwrap();
}