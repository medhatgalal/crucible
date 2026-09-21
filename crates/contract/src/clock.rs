/// Unix-seconds clock. Tests inject [`FixedClock`].
pub trait Clock {
    fn now_unix(&self) -> i64;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_unix(&self) -> i64 {
        match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            Ok(d) => d.as_secs() as i64,
            Err(_) => 0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FixedClock {
    unix: i64,
}

impl FixedClock {
    pub fn new(unix: i64) -> Self {
        Self { unix }
    }
}

impl Clock for FixedClock {
    fn now_unix(&self) -> i64 {
        self.unix
    }
}
