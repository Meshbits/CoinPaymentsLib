use std::net::{SocketAddr, Ipv4Addr};
use tonic::transport::Server;
use sapling::error::{WalletError};
use anyhow::Context;
use tonic::{Request, Response};
use sapling::db::{get_balance, DbPreparedStatements, generate_address, import_address, cancel_payment};
use postgres::{Client, NoTls};
use sapling::{CONNECTION_STRING, prepare_tx, db, scan_chain, ZcashdConf, get_latest_height, broadcast_tx};
use std::ops::DerefMut;
use std::sync::{Arc, Mutex};
use std::borrow::BorrowMut;
use tokio::runtime::Runtime;
use tokio::task::block_in_place;
use std::time::{SystemTime, Duration};
use rand::rngs::OsRng;
use rand::thread_rng;
use sapling::zams_rpc as grpc;
use sapling::zams_rpc::{PubKeyId, AccountCursor, AddressType, pub_key, BlockHeight};
use zcash_client_backend::address::RecipientAddress;
use zcash_primitives::consensus::TestNetwork;
use zcash_primitives::transaction::components::amount::DEFAULT_FEE;

struct ZAMS {
    config: ZcashdConf,
    client: Arc<Mutex<Client>>,
    statements: DbPreparedStatements,
}

impl ZAMS {
    pub fn new() -> ZAMS {
        let config = ZcashdConf::new();
        let connection = Client::connect(CONNECTION_STRING, NoTls).unwrap();
        let client = Arc::new(Mutex::new(connection));
        let statements = DbPreparedStatements::prepare(client.lock().unwrap().deref_mut()).unwrap();
        ZAMS {
            config,
            client,
            statements,
        }
    }
}

#[tonic::async_trait]
impl grpc::block_explorer_server::BlockExplorer for ZAMS {
    async fn get_version(
        &self,
        _request: Request<grpc::Empty>,
    ) -> Result<Response<grpc::VersionReply>, tonic::Status> {
        Ok(Response::new(grpc::VersionReply {
            version: "1.0".to_string(),
        }))
    }

    async fn validate_address(
        &self,
        request: Request<grpc::ValidateAddressRequest>,
    ) -> Result<Response<grpc::Boolean>, tonic::Status> {
        let request = request.into_inner();
        let valid = RecipientAddress::decode(&TestNetwork, &request.address).is_some();
        let rep = grpc::Boolean {
            value: valid
        };
        Ok(Response::new(rep))
    }

    async fn get_account_balance(
        &self,
        request: Request<grpc::GetAccountBalanceRequest>,
    ) -> Result<Response<grpc::Amount>, tonic::Status> {
        let request = request.into_inner();
        let balance = block_in_place(|| {
            let mut c = self.client.lock().unwrap();
            get_balance(c.deref_mut(), request.account, request.min_confirmations as i32)
        }).unwrap();
        Ok(Response::new(grpc::Amount {
            amount: balance as u64,
        }))
    }

    async fn prepare_unsigned_tx(
        &self,
        request: Request<grpc::PrepareUnsignedTxRequest>,
    ) -> Result<Response<grpc::UnsignedTx>, tonic::Status> {
        let request = request.into_inner();
        let unsigned_tx = block_in_place(|| {
            let mut c = self.client.lock().unwrap();
            let datetime = SystemTime::UNIX_EPOCH + Duration::from_secs(request.timestamp);
            prepare_tx(datetime, request.from_account, &request.to_address, request.change_account, request.amount as i64,
            c.deref_mut(), &self.statements, &mut OsRng)
        })?;
        Ok(Response::new(unsigned_tx))
    }

    async fn cancel_tx(
        &self,
        request: Request<grpc::PaymentId>
    ) -> Result<Response<grpc::Empty>, tonic::Status> {
        let request = request.into_inner();
        block_in_place(|| {
            let mut c = self.client.lock().unwrap();
            cancel_payment(c.deref_mut(), request.id)
        })?;
        Ok(Response::new(grpc::Empty {}))
    }

    async fn broadcast_signed_tx(
        &self,
        request: Request<grpc::SignedTx>,
    ) -> Result<Response<grpc::TxId>, tonic::Status> {
        let request = request.into_inner();
        let tx_id = block_in_place(|| {
            let mut c = self.client.lock().unwrap();
            broadcast_tx(c.deref_mut(), &request)
        })?;
        let rep = grpc::TxId {
            hash: tx_id
        };
        Ok(Response::new(rep))
    }

    async fn estimate_fee(
        &self,
        _request: Request<grpc::EstimateFeeRequest>,
    ) -> Result<Response<grpc::Fee>, tonic::Status> { 
        let fee = grpc::Fee {
            amount: u64::from(DEFAULT_FEE),
            perkb: false
        };
        Ok(Response::new(fee))
    }

    async fn get_current_height(
        &self,
        _request: Request<grpc::Empty>,
    ) -> Result<Response<grpc::BlockHeight>, tonic::Status> {
        let end_height = block_in_place(|| get_latest_height())?;
        let height = grpc::BlockHeight {
            height: end_height,
        };
        Ok(Response::new(height))
    }
    async fn sync(
        &self,
        _request: Request<grpc::Empty>,
    ) -> Result<Response<grpc::BlockHeight>, tonic::Status> {
        let end_height = block_in_place(|| {
            scan_chain(self.client.clone(), &self.config)
        })?;
        Ok(Response::new(grpc::BlockHeight {
            height: end_height
        }))
    }

    async fn rewind(
        &self,
        request: Request<grpc::BlockHeight>,
    ) -> Result<Response<grpc::Empty>, tonic::Status> {
        todo!()
    }

    async fn import_public_key(
        &self,
        request: Request<grpc::PubKey>,
    ) -> Result<Response<grpc::PubKeyId>, tonic::Status> {
        let request = request.into_inner();
        let id_fvk = block_in_place(|| {
            let mut client = self.client.lock().unwrap();
            match request.address_type {
                Some(pub_key::AddressType::Address(address)) => {
                    let id_account = import_address(client.deref_mut(), &address).unwrap();
                    Ok(id_account)
                }
                Some(pub_key::AddressType::Fvk(fvk)) => {
                    let id_fvk = db::import_fvk(client.deref_mut(), &fvk).unwrap();
                    Ok(id_fvk)
                }
                _ => return Err(WalletError::Error(anyhow::anyhow!("Invalid address type")))
            }
        })?;
        let rep = PubKeyId {
            id: id_fvk,
        };
        Ok(Response::new(rep))
    }

    async fn new_account(
        &self,
        request: Request<grpc::PubKeyCursor>,
    ) -> Result<Response<grpc::AccountCursor>, tonic::Status> {
        let request = request.into_inner();
        let diversifier_index = (request.diversifier_high as u128) << 64 | request.diversifier_low as u128;
        let account = block_in_place(|| {
            let mut client = self.client.lock().unwrap();
            let (id_account, address, di) = generate_address(client.deref_mut(), request.id_fvk, diversifier_index).unwrap();
            AccountCursor {
                id_account,
                address,
                diversifier_high: (di >> 64) as u64,
                diversifier_low: di as u64
            }
        });
        Ok(Response::new(account))
    }
}

fn main() {
    let port = 3001;
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);
    let exporer = ZAMS::new();
    let r = Runtime::new().unwrap();
    r.block_on(Server::builder()
        .add_service(grpc::block_explorer_server::BlockExplorerServer::new(exporer))
        .serve(addr)
    ).unwrap();
}
