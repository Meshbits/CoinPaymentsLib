use clap::Clap;
use rand::thread_rng;
use sapling::{broadcast_tx, load_checkpoint, prepare_tx, rewind_to_height, scan_chain, sign_tx, import_fvk};
use postgres::{NoTls, Client};
use sapling::{DbPreparedStatements, get_balance, import_address, generate_address, cancel_payment};
use std::time::SystemTime;
use std::sync::{Mutex, Arc};
use sapling::config::ZamsConfig;

#[derive(Clap)]
struct CommandArgs {
    #[clap(subcommand)]
    cmd: Command,
}

#[derive(Clap)]
enum Command {
    LoadCheckpoint {
        height: u32,
    },
    Rewind {
        height: u32,
    },
    Scan,
    ImportFVK {
        fvk: String,
    },
    ImportAddress {
        address: String,
    },
    GenerateNewAddress {
        id_fvk: i32,
    },
    GetBalance {
        account: i32,
        min_confirmations: Option<i32>,
    },
    PrepareTx {
        from_account: i32,
        to_address: String,
        change_account: i32,
        amount: i64,
    },
    CancelTx {
        id: i32,
    },
    SignTx {
        sk: String,
        unsigned_tx: String,
    },
    BroadcastTx {
        signed_tx: String,
    },
}

fn main() {
    let config = ZamsConfig::default();
    let connection = Client::connect(&config.connection_string, NoTls).unwrap();
    let c = Arc::new(Mutex::new(connection));
    let statements = DbPreparedStatements::prepare(&mut *c.lock().unwrap()).unwrap();
    let mut rng = thread_rng();

    let opts = CommandArgs::parse();
    let cmd = opts.cmd;
    match cmd {
        Command::LoadCheckpoint { height } => {
            let mut client = c.lock().unwrap();
            load_checkpoint(&mut *client, height, &config).unwrap();
        }
        Command::Rewind { height } => {
            rewind_to_height(c, height, &config).unwrap();
        }
        Command::Scan => {
            scan_chain(c, &config).unwrap();
        }
        Command::ImportFVK { fvk } => {
            let mut client = c.lock().unwrap();
            let id_fvk = import_fvk(&mut *client, &fvk).unwrap();
            println!("FVK {} imported as {}", fvk, id_fvk);
        }
        Command::ImportAddress { address } => {
            let mut client = c.lock().unwrap();
            let id_account = import_address(&mut *client, &address).unwrap();
            println!("Address {} imported as {}", address, id_account);
        }
        Command::GenerateNewAddress {
            id_fvk,
        } => {
            let mut client = c.lock().unwrap();
            let (id_account, addr) = generate_address(config.network, &mut *client, id_fvk).unwrap();
            println!("New account {} generated with address {}", id_account, &addr);
        }
        Command::GetBalance { account, min_confirmations } => {
            let mut client = c.lock().unwrap();
            let min_confirmations = min_confirmations.unwrap_or(1);
            let balance = get_balance(&mut *client, account, min_confirmations, &config).unwrap();
            println!("total = {} available = {}", balance.total, balance.available);
        }
        Command::PrepareTx { from_account, to_address, change_account, amount} => {
            let mut client = c.lock().unwrap();
            let tx =
                prepare_tx(config.network, SystemTime::now(), from_account, &to_address, change_account, amount, &mut *client, &statements, &mut rng).unwrap();
            println!("{}", serde_json::to_string(&tx).unwrap());
        }
        Command::CancelTx { id } => {
            let mut client = c.lock().unwrap();
            cancel_payment(&mut *client, id).unwrap();
        }
        Command::SignTx { sk, unsigned_tx } => {
            let unsigned_tx = serde_json::from_str(&unsigned_tx).unwrap();
            let signed_tx = sign_tx(config.network, &sk, unsigned_tx).unwrap();
            println!("{}", serde_json::to_string(&signed_tx).unwrap());
        }
        Command::BroadcastTx { signed_tx } => {
            let mut client = c.lock().unwrap();
            let signed_tx = serde_json::from_str(&signed_tx).unwrap();
            let txid = broadcast_tx(&mut *client, &signed_tx, &config).unwrap();
            println!("{}", txid);
        }
    }
}
