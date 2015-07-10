 - put description and objective in README
 - setup CI with travis

 - Rename pipe ready state and is_ready to something that does NOT match the ready fn of mio
 - Remove auto sent dummy msg
 - Make Message able to send itself through a TryWrite, consuming itself, returning OK or remaining bytes to be send ...
