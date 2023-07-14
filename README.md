<p align="center">
  <a href="https://decentraland.org">
    <img alt="Decentraland" src="https://decentraland.org/images/logo.png" width="60" />
  </a>
</p>
<h1 align="center">
  Camera Reel Service
</h1>

The Camera Reel Service is a simple solution designed specifically for uploading and retrieving camera images taken from Decentraland Explorer. This service enables users to capture and store images with additional metadata, providing valuable context to enhance their visual content.

# Setup

Before start, make sure you have these installed:
- **Rust** | you can use this [Development setup guide](https://www.notion.so/decentraland/Development-Setup-3ea6715744944d1cbab0bf569f329f06) 
- **docker-compose** | used for DB and MinIO
- **just** | A command runner - use `cargo install just` or follow the [Installation guide](https://github.com/casey/just#installation)

# Run

Before running the Camera Reel service you need Postgres and MinIO instances, you can start both by running:
```console
$ just run-services
```

In order to run the Camera Reel service:
```console
$ cargo run --bin camera-reel-service
```

Also, you can run it in watch mode by installing `cargo-watch` and using the command to run the server:
```console
$ cargo install cargo-watch
$ cargo watch -x 'run'
```

## Logging
The `RUST_LOG` environment variable can be used to specify the log level, for example:

```console
$ RUST_LOG=debug cargo run
```
_See [these docs](https://docs.rs/env_logger/latest/env_logger/) to understand the possible values._

# Architecture
Here is a highlevel architecture overview that can help to understand the project strucuture and components:

![Camera Reel service architecture](docs/architecture.svg)
