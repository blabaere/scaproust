current: Recv max buffer size & tcp no delay

core

- add recv max size & no delay to core::EndpointSpec
- remove id from core::EndpointSpec
- replace all fields except id in core::endpoint::Endpoint by an EndpointSpec
- add an id in Schedulable reconnect & rebind variant
- replace all relevant parameters of Network::[re]connect|bind by an EndpointSpec parameter

reactor

- fix adapter core::Network trait implementation 

transport

- add recv max size & no delay parameters to bind & connect, or create a struct for that
- ...


### Improvements
- Recv max buffer size
- Reconnect interval max 
- Linger
- Handle accept error
- Req prefetch replies
- Use a pool for payloads and buffers (if any)
- Find something more efficient than a channel for sending replies from the event loop back to the facade (a mailbox?)
- IPC transport on windows : See https://github.com/mmacedoeu/pipetoredis.rs
- INPROC transport : to be determined (rust channel's are probably doing a better work at this)

### Features
- Websocket transport
- TLS transport
- Implement nanocat
- STAR protocol ?
- Polling and non-blocking operations ? Maybe not, see below

### Vision
- Expose async io using future-rs ?

### Tasks
- Use github issues instead of this file
- Document release process
- Document contribution mode
- AUTOMATE ALL THE THINGS !!! (compat test, benchmark ...)
- Change copyright header to mention 'authors' and the AUTHORS file
- Check [travis-cargo](https://github.com/huonw/travis-cargo)
- Adds coverage to the build and display it


### Things to look at

gather/scatter io operations
https://github.com/seanmonstar/vecio

Windows named pipes
https://github.com/mmacedoeu/pipetoredis.rs

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
