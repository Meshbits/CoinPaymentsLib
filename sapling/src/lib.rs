pub const CONNECTION_STRING: &str = "host=localhost dbname=saplingdb user=hanh";

pub mod grpc {
    tonic::include_proto!("cash.z.wallet.sdk.rpc");
}

pub mod error;
pub mod wallet;

