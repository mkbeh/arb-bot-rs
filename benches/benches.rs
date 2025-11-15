use criterion::criterion_main;

mod binance;
mod kucoin;

criterion_main! {
    binance::order_benchmark::benches,
    kucoin::order_benchmark::benches,
}
