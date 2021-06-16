---
title: Build Instructions
date: "2021-06-16T12:08:00+08:00"
---

{{ page.date | date_to_long_string }}

## Requirements

- [Rust](https://www.rust-lang.org/)
- NodeJS [Yarn](https://yarnpkg.com/), it should probably work with NPM too
- [.NET Core 5](https://dotnet.microsoft.com/download/dotnet/5.0)
- [Postgres](https://www.postgresql.org/)
- [GRPCurl](https://github.com/fullstorydev/grpcurl)

# Build

## ZAMS & Signer

```sh
$ cd zams
$ cargo build --release
```

## CPP Integration Library

```sh
$ cd integration
$ dotnet build
```

## Mock CPP Notification Server

```sh
$ cd mock_notification_listener
$ yarn
```

## Database

- Create a database (for example saplingdb)
- Run the `zams/sql/up.sql` script to create the tables

```sh
psql -d template1 -c 'CREATE DATABASE saplingdb'
psql -d saplingdb <zams/sql/up.sql
```

# Configuration

- Copy or rename `zams-template.ini` to `zams.ini`
- Edit the database connection string in `zams.ini`
- Change `testnet` and `zcashd` if you want to connect to mainnet
- Change `port` if needed:

## Port 
Zams
- listens on `port` for GRPC connections,
- listens on `port+10` for Prometheus connections

Signer
- listens on `port+1` for GRPC connections

## Mainnet

Set `testnet` to false and change the `zcashd` URL. By default `zcashd` listens
on 18232 for testnet and 8232 for mainnet.

# Run

## Zcashd

- Edit `newblock.sh` and update the path and port to the ZAMS server
- Run `zcashd` with `blocknotify`

```sh
zcashd -blocknotify=zams/newblock.sh
```

## ZAMS

```sh
$ ./target/release/zams
```

## Signer

```sh
$ ./target/release/signer
```

## Mock Notification Listener

```sh
$ node index.js
```

# Tests

Tests are in the `integration/ZcashLib.Tests`

To run a specific test (for example KeyGeneration), use

```
$ dotnet test --filter DisplayName~KeyGeneration
```
