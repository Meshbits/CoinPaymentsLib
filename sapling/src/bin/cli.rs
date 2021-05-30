use clap::Clap;
use sapling::{generate_keys, import_fvk, scan, load_checkpoint, rewind_to_height};

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
    GenerateNewAddress {
        id_fvk: i32,
        diversifier_index: u128,
    },
}

fn main() {
    let opts = CommandArgs::parse();
    let cmd = opts.cmd;
    match cmd {
        Command::LoadCheckpoint { height } => {
            load_checkpoint(height).unwrap();
        }
        Command::Rewind {
            height
        } => {
            rewind_to_height(height).unwrap();
        }
        Command::Scan => {
            scan().unwrap();
        }
        Command::ImportFVK { fvk } => {
            let id_fvk = import_fvk(&fvk).unwrap();
            println!("{}", id_fvk);
        }
        Command::GenerateNewAddress {
            id_fvk,
            diversifier_index,
        } => {
            let (addr, di) = generate_keys(id_fvk, diversifier_index).unwrap();
            println!("{} {}", &addr, di);
        }
    }
}
