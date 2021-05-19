#!/bin/sh

grpcurl -import-path proto -proto api.proto -plaintext 127.0.0.1:9090 zams.BlockExplorer/GetVersion
