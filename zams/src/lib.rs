#[macro_use]
extern crate diesel;

pub mod grpc {
    tonic::include_proto!("zams");
}

pub mod zcashdrpc;
pub mod schema;
pub mod models;
pub mod db;
pub mod signer;
pub mod decrypt;
pub mod scanner;

#[cfg(test)]
pub mod testconfig;

