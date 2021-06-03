use bytes::Bytes;
use clap::Clap;
use rand::thread_rng;
use sapling::{broadcast_tx, load_checkpoint, prepare_tx, rewind_to_height, scan_chain, sign_tx, ZcashdConf, CONNECTION_STRING};
use sapling::error::WalletError::Postgres;
use postgres::{NoTls, Client};
use std::cell::RefCell;
use std::rc::Rc;
use sapling::db::{self, DbPreparedStatements, import_address, generate_keys};
use std::ops::DerefMut;

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
        diversifier_index: u128,
    },
    PrepareTx {
        from_account: i32,
        to_address: String,
        change_account: i32,
        amount: i64,
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
    let config = ZcashdConf::parse("http://127.0.0.1:18232", "/home/hanh/zcash-test").unwrap();
    let connection = Client::connect(CONNECTION_STRING, NoTls).unwrap();
    let c = Rc::new(RefCell::new(connection));
    let statements = DbPreparedStatements::prepare(c.clone()).unwrap();
    let mut rng = thread_rng();

    let opts = CommandArgs::parse();
    let cmd = opts.cmd;
    match cmd {
        Command::LoadCheckpoint { height } => {
            let mut client = c.borrow_mut();
            load_checkpoint(client.deref_mut(), height).unwrap();
        }
        Command::Rewind { height } => {
            rewind_to_height(height).unwrap();
        }
        Command::Scan => {
            scan_chain(c.clone(), &config).unwrap();
        }
        Command::ImportFVK { fvk } => {
            let mut client = c.borrow_mut();
            let id_fvk = db::import_fvk(client.deref_mut(), &fvk).unwrap();
            println!("{}", id_fvk);
        }
        Command::ImportAddress { address } => {
            let mut client = c.borrow_mut();
            let id_account = import_address(client.deref_mut(), &address).unwrap();
            println!("{}", id_account);
        }
        Command::GenerateNewAddress {
            id_fvk,
            diversifier_index,
        } => {
            let mut client = c.borrow_mut();
            let (addr, di) = generate_keys(client.deref_mut(), id_fvk, diversifier_index).unwrap();
            println!("{} {}", &addr, di);
        }
        Command::PrepareTx { from_account, to_address, change_account, amount} => {
            let mut client = c.borrow_mut();
            let tx =
                prepare_tx(from_account, &to_address, change_account, amount, client.deref_mut(), &statements, &mut rng).unwrap();
            println!("{}", serde_json::to_string(&tx).unwrap());
        }
        Command::SignTx { sk, unsigned_tx } => {
            let unsigned_tx = serde_json::from_str(&unsigned_tx).unwrap();
            let signed_tx = sign_tx(&sk, unsigned_tx).unwrap();
            println!("{}", hex::encode(signed_tx));
        }
        Command::BroadcastTx { signed_tx } => {
            let tx = Bytes::from(hex::decode(&signed_tx).unwrap());
            broadcast_tx(&tx).unwrap();
        }
    }
}
