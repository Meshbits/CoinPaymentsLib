use std::net::{SocketAddr, Ipv4Addr};
use tonic::transport::Server;
use sapling::error::{WalletError};
use anyhow::{anyhow};
use tonic::{Request, Response, Status};
use tokio::runtime::Runtime;

use sapling::{zams_rpc as grpc, get_bip39_seed, generate_sapling_keys, generate_transparent_address, sign_tx, ZamsConfig};
use sapling::zams_rpc::{Empty, VersionReply, Keys, Entropy, PubKey, pub_key, SignTxRequest, SignedTx};


struct Signer {
    config: ZamsConfig,
}
impl Signer {
    pub fn new(config: &ZamsConfig) -> Signer {
        Signer {
            config: config.clone()
        }
    }
}

#[tonic::async_trait]
impl grpc::signer_server::Signer for Signer {
    async fn get_version(&self, _request: Request<Empty>) -> Result<Response<VersionReply>, Status> {
        Ok(Response::new(VersionReply {
            version: "1.0".to_string()
        }))
    }

    async fn generate_transparent_key(&self, request: Request<Entropy>) -> Result<Response<Keys>, Status> {
        let request = request.into_inner();
        let seed = get_bip39_seed(request.clone())?;
        let (sk, address) = generate_transparent_address(self.config.network, seed, &request.path);
        let keys = Keys {
            pk: Some(PubKey { address_type: Some(pub_key::AddressType::Address(address)) }),
            sk
        };
        Ok(Response::new(keys))
    }

    async fn generate_sapling_key(&self, request: Request<Entropy>) -> Result<Response<Keys>, Status> {
        let request = request.into_inner();
        let seed = get_bip39_seed(request.clone())?;
        let (sk, fvk) = generate_sapling_keys(self.config.network, seed, &request.path);
        let keys = Keys {
            pk: Some(PubKey { address_type: Some(pub_key::AddressType::Fvk(fvk)) }),
            sk
        };
        Ok(Response::new(keys))
    }

    async fn sign_tx(&self, request: Request<SignTxRequest>) -> Result<Response<SignedTx>, Status> {
        let request = request.into_inner();
        let unsigned_tx = request.unsigned_tx.ok_or(WalletError::Error(anyhow!("Missing unsigned tx")))?;
        let signed_tx = sign_tx(self.config.network, &request.secret_key, unsigned_tx)?;
        Ok(Response::new(signed_tx))
    }
}

fn main() {
    let config = ZamsConfig::default();
    let port = config.port + 1;
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);
    let signer = Signer::new(&config);
    let r = Runtime::new().unwrap();
    r.block_on(Server::builder()
        .add_service(grpc::signer_server::SignerServer::new(signer))
        .serve(addr)
    ).unwrap();
}
