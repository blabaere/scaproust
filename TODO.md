### Urgent
- Activate back integration tests when stainless is fixed

### Improvements
- Close endpoint
- Recv max buffer size
- Reconnect interval max 
- Linger
- Handle accept error
- Make transports pluggable 
- Req prefetch replies
- Use a pool for payloads and buffers (if any)
- Find something for efficient than a channel for sending replies from the event loop back to the facade (a mailbox?)
- IPC transport : See [mio-uds](https://github.com/alexcrichton/mio-uds)
- INPROC transport : to be determined (rust channel's are probably doing a better work at this)

### Features
- Websocket transport
- TLS transport
- Implement nanocat
- STAR protocol ?
- Polling and non-blocking operations

### Tasks
- Use github issues instead of this file
- Document release process
- Document contribution mode
- AUTOMATE ALL THE THINGS !!! (compat test, benchmark ...)
- Change copyright header to mention 'authors' and the AUTHORS file
- Check [travis-cargo](https://github.com/huonw/travis-cargo)
- Adds coverage to the build and display it


### Things to look at

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
