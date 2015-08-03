
WIP:
Sending, the protocol should be dealing with the whole process :
 - Create a timeout
 - Select a pipe and transfer the sending
 - Raise a failure event if no pipe is available within the specified timeout
 - Raise a success event and cancel the timeout if enough pipes finished sending the message
 - Cancel the send operation when the timeout is reached


General:
 - put description and objective in README
 - setup CI with travis once there are some unit tests
 - setup CI with appveyor once mio is compatible with windows

Refactors:
 - change on_pipe_error to have the protocol return the dead pipe, and call close on it, before asking it addr
 - find a better name for socket_impl and session_impl
 - maybe the acceptor could create pipes instead of connections ?

Features:
 - Implement send timeout
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
