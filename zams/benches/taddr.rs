use criterion::{black_box, criterion_group, criterion_main, Criterion};

use zams::{ZamsConfig, TrpWallet, populate_taddr};
use std::sync::{Arc, Mutex};
use postgres::{Client, NoTls};

#[allow(dead_code)]
fn scan_block() {
    let config = ZamsConfig::default();
    let client = Client::connect(&config.connection_string, NoTls).unwrap();
    let client = Arc::new(Mutex::new(client));
    let mut wallet = TrpWallet::new(client.clone(), config.clone()).unwrap();
    wallet.load_transparent_addresses_from_db().unwrap();
    wallet.scan_range(1_400_000..1_400_001).unwrap();
}

fn taddr_benchmark(c: &mut Criterion) {
    c.bench_function("populate", |b| b.iter(
        || populate_taddr(black_box(10_000))
    ));
    // c.bench_function("scan_block", |b| b.iter(
    //     || scan_block()
    // ));
}

criterion_group!(benches, taddr_benchmark);
criterion_main!(benches);
