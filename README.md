# ATOF("4") = 4.0

Welcome to the jungle

## stammer

stammer is the equivalent of murmur. it routes clients' media and addresses
control packets. the crate also contains the implementation of the client task
that is meant to be ran on the client-side.

## stutter

a prototype implementation of a client of stammer. it currently does not have
any UI or anything. it kickstarts the stammer client task and a stillborn media
task with some early attempts at master audio with rust
