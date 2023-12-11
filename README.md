# Verilator Simulation Manager

This code allows to manage multiple simulations starting from the same point.

The Rust / server code can be compiled with.

```bash
cargo run

# OR

cargo build
```

The C++ / client code can be compiled with.

```bash
g++ main.cpp ForkClient.cpp Protocol.cpp -o fork_client
```