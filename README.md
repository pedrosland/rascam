# Rascam

Rust library for interacting with the Raspberry Pi Camera.

This provides a friendly, high level API over the [mmal-sys](https://crates.io/crates/mmal-sys) library.

There are three main components in this library:

* Info - Describe the attached camera.
* SimpleCamera - Aims to provide a simple, easy to use API.
* SeriousCamera - This API is very unstable and will likely change! Aims to expose the power of the `mmal-sys`'s camera while providing a safe Rust API.

## Documentation and examples

Please see the [documentation](https://pedrosland.github.io/rascam/) and [examples](https://github.com/pedrosland/rascam/tree/master/examples).

## Usage

Add the following to your Cargo.toml, changing `0.0.2` for the latest release:

```toml
[dependencies]
rascam = "0.0.2"
```

Check out the [SimpleCamera example](https://github.com/pedrosland/rascam/blob/master/examples/simple.rs) to get started quickly.

This crate uses [tracing](https://crates.io/crates/tracing) to produce logging events. 
If things are crashing or producing unexpected results, you can print the logs by adding the [tracing-subscriber](https://crates.io/crates/tracing-subscriber) crate to your project.
Enable the logging by putting `tracing_subscriber::fmt::init();` at the top of your `main()` function. Set the `RUST_LOG` environment variable to `DEBUG`.
For example, this can be done by calling cargo like this: `$ RUST_LOG=DEBUG cargo run`.

```toml
[dependencies]
rascam = { version = "0.0.1" }
```

## License

Released under the MIT license.
