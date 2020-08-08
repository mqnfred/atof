# ATOF("4") = 4.0

Welcome to the jungle

## Test it out

```
cargo run --package stutterd localhost:5001 >/tmp/stutterd.log 2>&1 &
cargo run --package stuttertui localhost:5001
```

## Coding style

Logging levels:

 - debug: feel-good stuff we want to read about
 - info: run-of-the-mill changes in state we want to know about
 - warn: something went wrong, but this is not terminal to the program
 - error: something went really wrong, the program will stop now
