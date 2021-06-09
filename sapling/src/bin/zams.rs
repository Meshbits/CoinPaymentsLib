use sapling::error::WalletError;
use std::net::{Ipv4Addr, SocketAddr};
use tonic::transport::Server;

use postgres::{Client, NoTls};
use sapling::{broadcast_tx, prepare_tx, scan_chain, ZamsConfig};
use sapling::{
    cancel_payment, generate_address, get_balance, get_latest_height, get_payment_info,
    import_address, import_fvk, list_pending_payments, rewind_to_height, DbPreparedStatements,
};
use sapling::{register_custom_metrics, REGISTRY};
use std::sync::{Arc, Mutex};
use tonic::{Request, Response};

use rand::rngs::OsRng;
use std::time::{Duration, SystemTime};
use tokio::runtime::Runtime;
use tokio::task::block_in_place;

use chrono::{DateTime, Local};
use flexi_logger::{Age, Cleanup, Criterion, Logger, Naming};
use sapling::zams_rpc as grpc;
use sapling::zams_rpc::*;
use sapling::REQUESTS;
use warp::{Filter, Rejection, Reply};
use zcash_client_backend::address::RecipientAddress;
use zcash_primitives::consensus::TestNetwork;
use zcash_primitives::transaction::components::amount::DEFAULT_FEE;

struct ZAMS {
    config: ZamsConfig,
    client: Arc<Mutex<Client>>,
    statements: DbPreparedStatements,
}

impl ZAMS {
    pub fn new() -> ZAMS {
        let config = ZamsConfig::default();
        let connection = Client::connect(&config.connection_string, NoTls).unwrap();
        let client = Arc::new(Mutex::new(connection));
        let statements = {
            let mut c = client.lock().unwrap();
            DbPreparedStatements::prepare(&mut *c).unwrap()
        };
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
        let rep = grpc::Boolean { value: valid };
        Ok(Response::new(rep))
    }

    async fn get_account_balance(
        &self,
        request: Request<grpc::GetAccountBalanceRequest>,
    ) -> Result<Response<grpc::Balance>, tonic::Status> {
        let request = request.into_inner();
        let balance = block_in_place(|| {
            let mut client = self.client.lock().unwrap();
            get_balance(
                &mut *client,
                request.account,
                request.min_confirmations as i32,
                &self.config,
            )
        })
        .unwrap();
        Ok(Response::new(balance))
    }

    async fn prepare_unsigned_tx(
        &self,
        request: Request<grpc::PrepareUnsignedTxRequest>,
    ) -> Result<Response<grpc::UnsignedTx>, tonic::Status> {
        let request = request.into_inner();
        let unsigned_tx = block_in_place(|| {
            let mut client = self.client.lock().unwrap();
            let datetime = SystemTime::UNIX_EPOCH + Duration::from_secs(request.timestamp);
            prepare_tx(
                self.config.network,
                datetime,
                request.from_account,
                &request.to_address,
                request.change_account,
                request.amount as i64,
                &mut *client,
                &self.statements,
                &mut OsRng,
            )
        })?;
        Ok(Response::new(unsigned_tx))
    }

    async fn cancel_tx(
        &self,
        request: Request<grpc::PaymentId>,
    ) -> Result<Response<grpc::Empty>, tonic::Status> {
        let request = request.into_inner();
        block_in_place(|| {
            let mut client = self.client.lock().unwrap();
            cancel_payment(&mut *client, request.id)
        })?;
        Ok(Response::new(grpc::Empty {}))
    }

    async fn list_pending_payments(
        &self,
        request: Request<AccountId>,
    ) -> Result<Response<PaymentIds>, tonic::Status> {
        let request = request.into_inner();
        let payment_ids = block_in_place(|| {
            let mut client = self.client.lock().unwrap();
            list_pending_payments(&mut *client, request.id)
        })?;
        let res = PaymentIds { ids: payment_ids };
        Ok(Response::new(res))
    }

    async fn get_payment_info(
        &self,
        request: Request<PaymentId>,
    ) -> Result<Response<Payment>, tonic::Status> {
        let request = request.into_inner();
        let payment = block_in_place(|| {
            let mut client = self.client.lock().unwrap();
            get_payment_info(&mut *client, request.id)
        })?;
        Ok(Response::new(payment))
    }

    async fn broadcast_signed_tx(
        &self,
        request: Request<grpc::SignedTx>,
    ) -> Result<Response<grpc::TxId>, tonic::Status> {
        let request = request.into_inner();
        let tx_id = block_in_place(|| {
            let mut client = self.client.lock().unwrap();
            broadcast_tx(&mut *client, &request, &self.config)
        })?;
        let rep = grpc::TxId { hash: tx_id };
        Ok(Response::new(rep))
    }

    async fn estimate_fee(
        &self,
        _request: Request<grpc::EstimateFeeRequest>,
    ) -> Result<Response<grpc::Fee>, tonic::Status> {
        let fee = grpc::Fee {
            amount: u64::from(DEFAULT_FEE),
            perkb: false,
        };
        Ok(Response::new(fee))
    }

    async fn get_current_height(
        &self,
        _request: Request<grpc::Empty>,
    ) -> Result<Response<grpc::BlockHeight>, tonic::Status> {
        let end_height = block_in_place(|| get_latest_height(&self.config))?;
        let height = grpc::BlockHeight { height: end_height };
        Ok(Response::new(height))
    }
    async fn sync(
        &self,
        _request: Request<grpc::Empty>,
    ) -> Result<Response<grpc::BlockHeight>, tonic::Status> {
        let end_height = block_in_place(|| scan_chain(self.client.clone(), &self.config))?;
        Ok(Response::new(grpc::BlockHeight { height: end_height }))
    }

    async fn rewind(
        &self,
        request: Request<grpc::BlockHeight>,
    ) -> Result<Response<grpc::Empty>, tonic::Status> {
        let request = request.into_inner();
        block_in_place(|| rewind_to_height(self.client.clone(), request.height, &self.config))?;
        Ok(Response::new(Empty {}))
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
                    let id_account = import_address(&mut *client, &address).unwrap();
                    Ok(id_account)
                }
                Some(pub_key::AddressType::Fvk(fvk)) => {
                    let id_fvk = import_fvk(&mut *client, &fvk).unwrap();
                    Ok(id_fvk)
                }
                _ => return Err(WalletError::Error(anyhow::anyhow!("Invalid address type"))),
            }
        })?;
        let rep = PubKeyId { id: id_fvk };
        Ok(Response::new(rep))
    }

    async fn new_account(
        &self,
        request: Request<grpc::PubKeyCursor>,
    ) -> Result<Response<grpc::AccountCursor>, tonic::Status> {
        let request = request.into_inner();
        let diversifier_index =
            (request.diversifier_high as u128) << 64 | request.diversifier_low as u128;
        let account = block_in_place(|| {
            let mut client = self.client.lock().unwrap();
            let (id_account, address, di) = generate_address(
                self.config.network,
                &mut *client,
                request.id_fvk,
                diversifier_index,
            )
            .unwrap();
            AccountCursor {
                id_account,
                address,
                diversifier_high: (di >> 64) as u64,
                diversifier_low: di as u64,
            }
        });
        Ok(Response::new(account))
    }

    async fn batch_new_accounts(
        &self,
        request: Request<BatchNewAccountsRequest>,
    ) -> Result<Response<AccountCursor>, tonic::Status> {
        let request = request.into_inner();
        let pubkey_cursor = request.pubkey_cursor.unwrap();
        let mut diversifier_index =
            (pubkey_cursor.diversifier_high as u128) << 64 | pubkey_cursor.diversifier_low as u128;
        let count = request.count as usize;
        let account = block_in_place(|| {
            let mut client = self.client.lock().unwrap();
            let mut account_cursor = None;
            for _ in 0..count {
                let (id_account, address, di) = generate_address(
                    self.config.network,
                    &mut *client,
                    pubkey_cursor.id_fvk,
                    diversifier_index,
                )
                .unwrap();
                diversifier_index = di;
                account_cursor = Some(AccountCursor {
                    id_account,
                    address,
                    diversifier_high: (di >> 64) as u64,
                    diversifier_low: di as u64,
                });
            }
            account_cursor
        });
        Ok(Response::new(
            account.ok_or_else(|| WalletError::from(anyhow::anyhow!("")))?,
        ))
    }
}

async fn metrics_handler() -> Result<impl Reply, Rejection> {
    use prometheus::Encoder;
    let encoder = prometheus::TextEncoder::new();

    let mut buffer = Vec::new();
    if let Err(e) = encoder.encode(&REGISTRY.gather(), &mut buffer) {
        eprintln!("could not encode custom metrics: {}", e);
    };
    let mut res = match String::from_utf8(buffer.clone()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("custom metrics could not be from_utf8'd: {}", e);
            String::default()
        }
    };
    buffer.clear();

    let mut buffer = Vec::new();
    if let Err(e) = encoder.encode(&prometheus::gather(), &mut buffer) {
        eprintln!("could not encode prometheus metrics: {}", e);
    };
    let res_custom = match String::from_utf8(buffer.clone()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("prometheus metrics could not be from_utf8'd: {}", e);
            String::default()
        }
    };
    buffer.clear();

    res.push_str(&res_custom);
    Ok(res)
}

fn perfcounter_interceptor(req: Request<()>) -> Result<Request<()>, tonic::Status> {
    REQUESTS.inc();
    Ok(req)
}

fn main() {
    Logger::with_str("info")
        .log_to_file()
        .directory("logs")
        .rotate(
            Criterion::Age(Age::Day),
            Naming::Timestamps,
            Cleanup::KeepLogFiles(7),
        )
        .start()
        .unwrap();

    register_custom_metrics();
    let metrics_route = warp::path!("metrics").and_then(metrics_handler);

    let now = SystemTime::now();
    let dt: DateTime<Local> = now.into();
    log::info!("ZAMS started on {}", dt.to_rfc2822());
    let config = ZamsConfig::default();
    let port = config.port;
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);
    let explorer = ZAMS::new();
    let r = Runtime::new().unwrap();

    r.spawn(warp::serve(metrics_route).run(SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port + 10)));

    r.block_on(
        Server::builder()
            .add_service(
                grpc::block_explorer_server::BlockExplorerServer::with_interceptor(
                    explorer,
                    perfcounter_interceptor,
                ),
            )
            .serve(addr),
    )
    .unwrap();
}
