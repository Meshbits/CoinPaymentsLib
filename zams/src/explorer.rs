use clap::Clap;
use flexi_logger::{Age, Criterion, LogTarget, Logger, Naming, Cleanup, detailed_format};
use std::net::{Ipv4Addr, SocketAddr};
use std::result::Result;
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use zams::grpc::{
    block_explorer_server::{BlockExplorer, BlockExplorerServer},
    *
};
use log::*;
use zams::zcashdrpc::ZcashdConf;

#[derive(Clap)]
struct CommandArgs {
    #[clap(short, long, default_value = "9090")]
    port: u16,
    zcashd_url: String,
    datadir: String,
}

pub struct Explorer {}

impl Explorer {
    pub fn new() -> Explorer {
        Explorer {}
    }
}

#[tonic::async_trait]
//noinspection RsUnresolvedReference
impl BlockExplorer for Explorer {
    async fn get_version(
        &self,
        _req: Request<zams::grpc::Empty>,
    ) -> Result<Response<VersionReply>, Status> {
        info!("Get version");
        Ok(Response::new(VersionReply {
            version: "0.1".to_string(),
        }))
    }

    async fn validate_address(&self, _req: Request<ValidateAddressRequest>) -> Result<Response<Boolean>, Status> { todo!() }
    async fn get_address_balance(&self, _req: Request<GetAddressBalanceRequest>) -> Result<Response<Amount>, Status> { todo!() }
    async fn prepare_unsigned_tx(&self, _req: Request<PrepareUnsignedTxRequest>) -> Result<Response<RawTx>, Status> { todo!() }
    async fn broadcast_signed_tx(&self, _req: Request<RawTx>) -> Result<Response<TxId>, Status> { todo!() }
    async fn estimate_fee(&self, _req: Request<EstimateFeeRequest>) -> Result<Response<Fee>, Status> { todo!() }
    async fn get_current_height(&self, _req: Request<Empty>) -> Result<Response<BlockHeight>, Status> { todo!() }
    async fn get_tx_info(&self, _req: Request<TxId>) -> Result<Response<TxInfo>, Status> { todo!() }
    async fn rescan(&self, _req: Request<BlockHeight>) -> Result<Response<BlockHeight>, Status> { todo!() }
    async fn import_public_key(&self, _req: Request<PubKey>) -> Result<Response<Empty>, Status> { todo!() }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Logger::with_env_or_str("info")
        .log_target(LogTarget::File)
        .directory("log")
        .buffer_and_flush()
        .format(detailed_format)
        .rotate(
            Criterion::Age(Age::Day),
            Naming::Timestamps,
            Cleanup::KeepLogAndCompressedFiles(7, 14),
        )
        .print_message()
        .start()?;
    let opts = CommandArgs::parse();
    let port = opts.port;
    let zcashd_url = opts.zcashd_url;
    let datadir = opts.datadir;
    let _config = ZcashdConf::parse(&zcashd_url, &datadir)?;

    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);
    let exporer = Explorer::new();
    Server::builder()
        .add_service(BlockExplorerServer::new(exporer))
        .serve(addr)
        .await?;

    Ok(())
}
