use criterion::criterion_main;
mod exchange;

criterion_main! {
    exchange::order_benchmark::benches,
}
