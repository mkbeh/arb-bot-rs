use std::sync::{LazyLock, Mutex};

static CLOSER: LazyLock<Mutex<Closer>> = LazyLock::new(|| Mutex::new(Closer::default()));

/// Add callback to global closer array.
pub fn push_callback(callback: CloserFunc<'static>) {
    CLOSER.lock().unwrap().push(callback);
}

/// Execute all added callbacks to global closer array.
pub fn cleanup_resources() {
    CLOSER.lock().unwrap().close()
}

type CloserFunc<'a> = Box<dyn Fn() + 'a + Send + Sync>;

#[derive(Default)]
struct Closer<'a> {
    closers: Vec<CloserFunc<'a>>,
}

impl<'a> Closer<'a> {
    fn push(&mut self, callback: CloserFunc<'a>) {
        self.closers.push(callback);
    }
    fn close(&mut self) {
        self.closers.iter_mut().for_each(|cb| cb());
    }
}
