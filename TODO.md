Taken from the source: https://github.com/carllerche/mio/blob/getting-started/doc/getting-started.md
  Important: Even if we just received a ready notification, there is no guarantee that a read from the socket will succeed and not return Ok(None), so we must handle that case as well.
That sounds pretty bad, maybe this case should be handled, whenever a recv operation is started,
if the first result is the so-called Ok(None), WouldBlock currently, the operation should be cancelled.
And the socket owning the pipe should be notified so it can reschedule the operation.

PB: the dispatcher receiving and CanSend/CanRecv events does not know if a device or a probe is 'listening'. What if several probes are interested in the readiness the same socket ?

IDEA: maybe pipe should raise CanSend/Recv(bool) instead of just CanSend/Recv ?

Change doc links of versioned packaged to docs.rs, since it is easy to support several version.
See https://docs.rs/about

### Improvements
- Reconnect interval max 
- Linger
- Handle accept error
- Req prefetch replies
- Use a pool for payloads and buffers (if any)
- Find something more efficient than a channel for sending replies from the event loop back to the facade (a mailbox?)
- INPROC transport : to be determined (rust channel's are probably doing a better work at this)
  

### Features
- Websocket transport
- TLS transport
- Implement nanocat
- STAR protocol ?
  

### Vision
- Expose async io using future-rs ?
- Change the facade API to have "typed sockets" ?
  

### Tasks
- Use github issues instead of this file
- Document release process
- Document contribution mode
- AUTOMATE ALL THE THINGS !!! (compat test, benchmark ...)


### Things to look at

gather/scatter io operations
https://github.com/seanmonstar/vecio

https://pascalhertleif.de/artikel/good-practices-for-writing-rust-libraries/
http://keepachangelog.com

https://github.com/tokio-rs
**mioco now has a timeout feature !**  
https://github.com/dpc/mioco  
https://github.com/dpc/mioco/blob/master/examples%2Fechoplus.rs  


https://github.com/frankmcsherry/recycler
https://github.com/zslayton/lifeguard
http://carllerche.github.io/pool/pool/


https://github.com/tailhook/rotor  
https://github.com/dwrensha/gj  
https://github.com/zonyitoo/simplesched  
https://github.com/alexcrichton/wio (for appveyor ci script and doc publication too)  


Websocket
https://github.com/housleyjk/ws-rs  
https://github.com/cyderize/rust-websocket  
