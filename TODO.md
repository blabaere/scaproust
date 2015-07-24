 - put description and objective in README
 - setup CI with travis once there are some unit tests

 - Implement send timeout
 - Implement Pull protocol to see how receive operation can be done
 - Implement Bind operation
 - Now that there is send & receive, connect & bind : TEST ALL THE THINGS !!!
 - Implement the other protocols ...
 - Implement socket options ...
 - Implement load balancing and fair queuing
 - Have Socket::connect return an Endpoint that can be shut down
 - Implement nanocat

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
