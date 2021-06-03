use bytes::Bytes;
use clap::Clap;
use rand::thread_rng;
use sapling::{broadcast_tx, generate_keys, import_address, import_fvk, load_checkpoint, prepare_tx, rewind_to_height, scan, sign_tx, ZcashdConf};

#[derive(Clap)]
struct CommandArgs {
    #[clap(subcommand)]
    cmd: Command,
}

#[derive(Clap)]
enum Command {
    LoadCheckpoint {
        height: i32,
    },
    Rewind {
        height: i32,
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
    let mut rng = thread_rng();
    let opts = CommandArgs::parse();
    let cmd = opts.cmd;
    match cmd {
        Command::LoadCheckpoint { height } => {
            load_checkpoint(height).unwrap();
        }
        Command::Rewind { height } => {
            rewind_to_height(height).unwrap();
        }
        Command::Scan => {
            scan(&config).unwrap();
        }
        Command::ImportFVK { fvk } => {
            let id_fvk = import_fvk(&fvk).unwrap();
            println!("{}", id_fvk);
        }
        Command::ImportAddress { address } => {
            let id_account = import_address(&address).unwrap();
            println!("{}", id_account);
        }
        Command::GenerateNewAddress {
            id_fvk,
            diversifier_index,
        } => {
            let (addr, di) = generate_keys(id_fvk, diversifier_index).unwrap();
            println!("{} {}", &addr, di);
        }
        Command::PrepareTx { from_account, to_address, change_account, amount} => {
            let tx =
                prepare_tx(from_account, &to_address, change_account, amount, &mut rng).unwrap();
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
