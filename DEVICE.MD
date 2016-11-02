This document explains how a device can work with stateful protocols like REQ/REP.
It's focus is on how the reply flows from the REP back to the original REQ while keeping the reqId.

User -> 
([][req-body])
REQ::send, pending=rid ->  // REQ generates a rid, appends it to the header, saves it as pending_rid
([rid][req-body])
NETWORK ->
([][rid;req-body])
D-REP::recv ->       // D-REP appends the eid to the header
([eid1][rid;req-body])
D-REP::recv ->       // D-REP moves the msg's head from body to the header
([eid1;rid][req-body])
D-REP::recv ->       // D-REP does NOT store the header as a backtrace, because it is RAW
([eid1;rid][req-body])
DEVICE ->            // DEVICE forwards it untouched, that is with a header
([eid1;rid][req-body])
D-REQ::send ->       // D-REQ does NOT generate a rid or store anything, because it is RAW
NETWORK ->
([][eid1;rid;req-body])
REP::recv ->         // REP appends the eid to the header
([eid2][eid1;rid;req-body])
REP::recv ->         // REP moves the msg's head from body to the header
([eid2;eid1;rid][req-body])
REP::recv, bt=eid2;eid1;rid ->       // REP moves the header to its own backtrace, because it is NOT RAW
([][req-body])
User -> 
HERE THE REP USER CODE IS PRESENTED AN UNTOUCHED req-body
AND NOW IT WILL SEND A REPLY rep-body
User ->
([][rep-body])
REP::send ->         // REP moves its backtrace to the message header, because it is NOT RAW
([eid2;eid1;rid][rep-body])
REP::send ->         // REP removes 4 bytes from header to find an endpoint
([eid1;rid][rep-body])
NETWORK ->
([][eid1;rid;rep-body])
D-REQ::recv ->       // D-REQ moves 4 bytes from the body to the header
([eid1][rid;rep-body])
D-REQ::recv ->       // D-REQ does not check the pending rid, because it is RAW
([eid1][rid;rep-body])
DEVICE ->            // DEVICE forwards it untouched, that is with a header
([eid1][rid;rep-body])
D-REP::send ->       // D-REP does nothing with its backtrace, because it is RAW
([eid1][rid;rep-body])
D-REP::send ->       // D-REP removes 4 bytes from header to find an endpoint
([][rid;rep-body])
NETWORK ->
([][rid;rep-body])
REQ::recv ->         // REQ moves 4 bytes from the body to the header
([rid][rep-body])
REQ::recv ->         // REQ checks the header against its own pending_rid
([rid][rep-body])
