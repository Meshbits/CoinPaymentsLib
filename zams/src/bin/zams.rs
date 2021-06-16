use zams::error::WalletError;
use std::net::{Ipv4Addr, SocketAddr};
use tonic::transport::Server;

use postgres::{Client, NoTls};
use zams::{broadcast_tx, prepare_tx, scan_chain, ZamsConfig};
use zams::{
    cancel_payment, generate_address, get_balance, get_latest_height, get_payment_info,
    import_address, import_fvk, list_pending_payments, rewind_to_height, DbPreparedStatements,
};
use zams::{register_custom_metrics, metrics_handler, REQUESTS};
use std::sync::{Arc, Mutex};
use tonic::{Request, Response, Status};

use rand::rngs::OsRng;
use std::time::{Duration, SystemTime};
use tokio::runtime::Runtime;
use tokio::task::block_in_place;

use chrono::{DateTime, Local};
use flexi_logger::{Age, Cleanup, Criterion, Logger, Naming};
use zams::zams_rpc as grpc;
use zcash_client_backend::address::RecipientAddress;
use zcash_primitives::transaction::components::amount::DEFAULT_FEE;
use warp::Filter;

struct ZAMS {
    config: ZamsConfig,
    client: Arc<Mutex<Client>>,
    statements: DbPreparedStatements,
    data_mutex: Mutex<()>,
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
            data_mutex: Mutex::new(()),
        }
    }
}

#[tonic::async_trait]
impl grpc::block_explorer_server::BlockExplorer for ZAMS {
    async fn get_version(
        &self,
        _request: Request<grpc::Empty>,
    ) -> Result<Response<grpc::VersionReply>, Status> {
        Ok(Response::new(grpc::VersionReply {
            version: zams::VERSION.to_string(),
        }))
    }

    async fn validate_address(
        &self,
        request: Request<grpc::ValidateAddressRequest>,
    ) -> Result<Response<grpc::Boolean>, Status> {
        let request = request.into_inner();
        let valid = RecipientAddress::decode(self.config.network, &request.address).is_some();
        let rep = grpc::Boolean { value: valid };
        Ok(Response::new(rep))
    }

    async fn get_account_balance(
        &self,
        request: Request<grpc::GetAccountBalanceRequest>,
    ) -> Result<Response<grpc::Balance>, Status> {
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
    ) -> Result<Response<grpc::UnsignedTx>, Status> {
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
    ) -> Result<Response<grpc::Empty>, Status> {
        let request = request.into_inner();
        block_in_place(|| {
            let mut client = self.client.lock().unwrap();
            cancel_payment(&mut *client, request.id)
        })?;
        Ok(Response::new(grpc::Empty {}))
    }

    async fn list_pending_payments(
        &self,
        request: Request<grpc::AccountId>,
    ) -> Result<Response<grpc::PaymentIds>, Status> {
        let request = request.into_inner();
        let payment_ids = block_in_place(|| {
            let mut client = self.client.lock().unwrap();
            list_pending_payments(&mut *client, request.id)
        })?;
        let res = grpc::PaymentIds { ids: payment_ids };
        Ok(Response::new(res))
    }

    async fn get_payment_info(
        &self,
        request: Request<grpc::PaymentId>,
    ) -> Result<Response<grpc::Payment>, Status> {
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
    ) -> Result<Response<grpc::TxId>, Status> {
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
    ) -> Result<Response<grpc::Fee>, Status> {
        let fee = grpc::Fee {
            amount: u64::from(DEFAULT_FEE),
            perkb: false,
        };
        Ok(Response::new(fee))
    }

    async fn get_current_height(
        &self,
        _request: Request<grpc::Empty>,
    ) -> Result<Response<grpc::BlockHeight>, Status> {
        let end_height = block_in_place(|| get_latest_height(&self.config))?;
        let height = grpc::BlockHeight { height: end_height };
        Ok(Response::new(height))
    }
    async fn sync(
        &self,
        _request: Request<grpc::Empty>,
    ) -> Result<Response<grpc::BlockHeight>, Status> {
        let _lock = self.data_mutex.lock();
        let end_height = block_in_place(|| scan_chain(self.client.clone(), &self.config))?;
        Ok(Response::new(grpc::BlockHeight { height: end_height }))
    }

    async fn rewind(
        &self,
        request: Request<grpc::BlockHeight>,
    ) -> Result<Response<grpc::Empty>, Status> {
        let _lock = self.data_mutex.lock();
        let request = request.into_inner();
        block_in_place(|| rewind_to_height(self.client.clone(), request.height, &self.config))?;
        Ok(Response::new(grpc::Empty {}))
    }

    async fn import_public_key(
        &self,
        request: Request<grpc::PubKey>,
    ) -> Result<Response<grpc::PubKeyId>, Status> {
        let request = request.into_inner();
        let id_fvk = block_in_place(|| {
            let mut client = self.client.lock().unwrap();
            match request.type_of_address {
                Some(grpc::pub_key::TypeOfAddress::Address(address)) => {
                    let id_account = import_address(&mut *client, &address).unwrap();
                    Ok(id_account)
                }
                Some(grpc::pub_key::TypeOfAddress::Fvk(fvk)) => {
                    let id_fvk = import_fvk(&mut *client, &fvk).unwrap();
                    Ok(id_fvk)
                }
                _ => Err(WalletError::Error(anyhow::anyhow!("Invalid address type"))),
            }
        })?;
        let rep = grpc::PubKeyId { id: id_fvk };
        Ok(Response::new(rep))
    }

    async fn new_account(
        &self,
        request: Request<grpc::PubKeyId>,
    ) -> Result<Response<grpc::AccountAddress>, Status> {
        let request = request.into_inner();
        let account = block_in_place(|| {
            let mut client = self.client.lock().unwrap();
            let (id_account, address) = generate_address(
                self.config.network,
                &mut *client,
                request.id
            )?;
            Ok::<_, WalletError>(grpc::AccountAddress {
                id_account,
                address,
            })
        })?;
        Ok(Response::new(account))
    }

    async fn batch_new_accounts(
        &self,
        request: Request<grpc::BatchNewAccountsRequest>,
    ) -> Result<Response<grpc::Empty>, Status> {
        let request = request.into_inner();
        let count = request.count as usize;
        block_in_place(|| {
            let mut client = self.client.lock().unwrap();
            for _ in 0..count {
                generate_address(
                    self.config.network,
                    &mut *client,
                    request.id_pubkey
                )
                .unwrap();
            }
            Ok::<_, WalletError>(())
        })?;
        Ok(Response::new(grpc::Empty {}))
    }
}

fn perfcounter_interceptor(req: Request<()>) -> Result<Request<()>, Status> {
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
