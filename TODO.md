General:
 - put description and objective in README
 - setup CI with travis once there are some unit tests
 - setup documentation generation and site and github pages
 - setup CI with appveyor once mio is compatible with windows

Refactors:
 - make the timeout enum hold a socket id rather than a token ?
 - Remove association between token and socketid when a pipe is dead
 - Register to readable or writable only when required, that is when operation status is in progress ? 
 - find a better name for socket_impl and session_impl
 - maybe the acceptor could create pipes instead of connections ?
 - Implement Read & Write trait on sockets
 - Use a pool for payloads

Features:
 - Check what to do when send/recv timeout is reached and parts of the message has already been transfered !
 - Now that there is send & receive, connect & bind : TEST ALL THE THINGS !!!
 - Implement the other protocols ...
 - Implement socket options ...
 - Implement load balancing and fair queuing
 - Have Socket::connect & bind return an Endpoint that can be shut down
 - Implement device
 - Implement nanocat


Stuff to look at :
https://github.com/dpc/mioco
https://github.com/dwrensha/gj
https://github.com/calc0000/tcp-loop
https://github.com/diwic/fdringbuf-rs
https://github.com/zonyitoo/simplesched
