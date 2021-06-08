use configparser::ini::Ini;
use zcash_primitives::consensus::Network;
use zcash_primitives::consensus::Network::{TestNetwork, MainNetwork};

#[derive(Debug, Clone)]
pub struct ZamsConfig {
    pub network: &'static Network,
    pub zcashd: String,
    pub rpc_user: String,
    pub rpc_password: String,
    pub port: u16,
    pub connection_string: String,
}

impl ZamsConfig {
    pub fn new(config_path: &str) -> ZamsConfig {
        let mut conf = Ini::new();
        conf.load(config_path).unwrap();
        let zcashd = conf.get("zams", "zcashd").unwrap();
        let rpc_user = conf.get("zams", "rpcuser").unwrap();
        let rpc_password = conf.get("zams", "rpcpassword").unwrap();
        let port = conf.getuint("zams", "port").unwrap().unwrap() as u16;
        let connection_string = conf.get("zams", "connection_string").unwrap();
        let testnet = conf.getbool("zams", "testnet").unwrap().unwrap_or(false);
        let network = if testnet { &TestNetwork } else { &MainNetwork };
        ZamsConfig {
            network,
            zcashd,
            rpc_user,
            rpc_password,
            port,
            connection_string,
        }
    }
}

impl Default for ZamsConfig {
    fn default() -> Self {
        ZamsConfig::new("zams.ini")
    }
}
