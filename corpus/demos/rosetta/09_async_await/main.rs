use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DELAY_MS: u64 = 5;

async fn step_one_after_delay(delay_ms: u64) -> i32 {
    let unix_before = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();

    thread::sleep(Duration::from_millis(delay_ms));

    let unix_after = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();

    assert!(unix_after >= unix_before);
    1
}

async fn run() -> i32 {
    let started = Instant::now();
    let value = step_one_after_delay(DELAY_MS).await;
    assert!(started.elapsed() >= Duration::from_millis(1));
    value + 1
}

fn dummy_raw_waker() -> RawWaker {
    fn clone(_: *const ()) -> RawWaker {
        dummy_raw_waker()
    }

    fn wake(_: *const ()) {}
    fn wake_by_ref(_: *const ()) {}
    fn drop(_: *const ()) {}

    RawWaker::new(
        std::ptr::null(),
        &RawWakerVTable::new(clone, wake, wake_by_ref, drop),
    )
}

fn block_on<F: Future>(future: F) -> F::Output {
    let waker = unsafe { Waker::from_raw(dummy_raw_waker()) };
    let mut future = Box::pin(future);
    let mut cx = Context::from_waker(&waker);

    loop {
        match Pin::as_mut(&mut future).poll(&mut cx) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

fn main() {
    let result = block_on(run());
    println!("{result}");
}
