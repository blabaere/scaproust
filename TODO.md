General:
 - put description and objective in README
 - setup CI with travis once there are some unit tests
 - setup CI with appveyor once mio is compatible with windows

Problem :
 - the session_impl receives notifications from the event loop with a token
 - a token identifies a connection or an acceptor
 - the notification should be forwarded to the socket owning this connection or acceptor (so the session needs to find a socket from a token)
 - an acceptor can create connections and the socket needs to create a token for each one of them.
 - when recreating a broken connection, the old token should be reused

NO : the socket operations should return the created/destroyed tokens so the session can update the socket id / token association.
 - connect should return Some<Token>, in case the connection has been created
 - listen should return Some<Token>, in case the listener has been created
 - ready should return Some<Vec<Token>>, in case a listener has accepted connections
 - when an error occurs while calling a connection/listener, the association should be destroyed ...
 ... and a Reconnect/Rebind timeout should be set on the event loop

Protocol is not concerned by Listeners and accept operation, it should only deal with connections.
So maybe listeners should be owned by the socket.

Refactors:
 - Use the sequence generator from within the socket ?
 - Use an mio::Token instead of usize where applicable
 - Use something else than usize where mio::Token is not applicable
 - find a better name for socket_impl and session_impl


Features:
 - Implement send timeout
 - Implement Pull protocol to see how receive operation can be done
 - Implement Bind operation
 - Now that there is send & receive, connect & bind : TEST ALL THE THINGS !!!
 - Implement the other protocols ...
 - Implement socket options ...
 - Implement load balancing and fair queuing
 - Have Socket::connect return an Endpoint that can be shut down
 - Implement nanocat


WIP:
Sending the protocol should be dealing with the whole process :
 - Create a timeout
 - Select a pipe and transfer the sending
 - Raise a failure event if no pipe is available within the specified timeout
 - Raise a success event and cancel the timeout if enough pipes finished sending the message
 - Cancel the send operation when the timeout is reached

The pipe should :
 - transfer the bytes, returning the progress made
 - check if the send request had the non-blocking option
 - notify the protocol that a message was sent (arg in function ? callback ? return result ?)

CURRENTLY THE PIPE IS RAISING THE SUCCESS EVENT
THIS IS WRONG BECAUSE THE PROTOCOL MAY REQUIRE 
THAT ALL LIVE PIPES SEND THE MESSAGE.
FOR EXAMPLE PUB, BUS, SURVEYOR ...


Stuff to look at :
https://github.com/dpc/mioco
https://github.com/dwrensha/gj
https://github.com/calc0000/tcp-loop
https://github.com/diwic/fdringbuf-rs
https://github.com/zonyitoo/simplesched
