# Rascam

Rust library for interacting with the Raspberry Pi Camera.

This provides a friendly, high level API over the [mmal-sys](https://crates.io/crates/mmal-sys) library.

There are three main components in this library:

* Info - Describe the attached camera.
* SimpleCamera - Aims to provide a simple, easy to use API.
* SeriousCamera - This API is very unstable and will likely change! Aims to expose the power of the `mmal-sys`'s camera while providing a safe Rust API.

## Documentation and examples

Please see the [documentation](https://docs.rs/crate/mmal/0.0.0) and [examples](https://github.com/pedrosland/mmal/tree/master/examples)

## Usage

Add the following to your Cargo.toml, changing `0.0.1` for the latest release:

```toml
[dependencies]
rascam = "0.0.1"
```

Import this crate into your lib.rs or main.rs file:

```rust
extern crate rascam;
```

If things are crashing or producing unexpected results there is a feature flag which enables some print statements which may help to debug an issue:

```toml
[dependencies]
libc = { version = "0.0.1", features = ["debug"] }
```

## License

Released under the MIT license.
