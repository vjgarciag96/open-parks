use jiff::Timestamp;

pub trait Clock: Send + Sync + 'static {
    fn now(&self) -> Timestamp;
}

impl<F: Fn() -> Timestamp + Send + Sync + 'static> Clock for F {
    fn now(&self) -> Timestamp {
        self()
    }
}
