use criterion::criterion_main;
mod exchange;

criterion_main! {
    crate::exchange::order_benchmark::benches,
}
