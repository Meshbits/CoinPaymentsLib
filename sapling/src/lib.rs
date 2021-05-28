pub mod grpc {
    tonic::include_proto!("cash.z.wallet.sdk.rpc");
}

mod decrypt;
mod init;

