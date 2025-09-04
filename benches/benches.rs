use criterion::criterion_main;

mod binance;

criterion_main! {
    binance::order_benchmark::benches,
}
