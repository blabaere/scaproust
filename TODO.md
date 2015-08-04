What to do when a pipe becomes readable while no op in progress: ignore
General:
 - put description and objective in README
 - setup CI with travis once there are some unit tests
 - setup CI with appveyor once mio is compatible with windows

Refactors:
 - make the timeout enum hold a socket id rather than a token ?
 - find a better name for socket_impl and session_impl
 - maybe the acceptor could create pipes instead of connections ?

Features:
 - Implement send timeout
 - Check what could be done when send timeout is reached and parts of the message has already been sent !
 - Implement Pull protocol to see how receive operation can be done
 - Now that there is send & receive, connect & bind : TEST ALL THE THINGS !!!
 - Implement the other protocols ...
 - Implement socket options ...
 - Implement load balancing and fair queuing
 - Have Socket::connect return an Endpoint that can be shut down
 - Implement device
 - Implement nanocat


Stuff to look at :
https://github.com/dpc/mioco
https://github.com/dwrensha/gj
https://github.com/calc0000/tcp-loop
https://github.com/diwic/fdringbuf-rs
https://github.com/zonyitoo/simplesched
