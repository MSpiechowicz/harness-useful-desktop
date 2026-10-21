//! Polling a batch of futures to completion together.

use std::future::poll_fn;
use std::task::Poll;

use super::BoxFuture;

/// Polls every future to completion concurrently and returns their outputs in order.
pub(crate) async fn join_all<T>(futures: Vec<BoxFuture<'_, T>>) -> Vec<T> {
    let mut pending: Vec<Option<BoxFuture<'_, T>>> = futures.into_iter().map(Some).collect();
    let mut outputs: Vec<Option<T>> = pending.iter().map(|_| None).collect();

    poll_fn(|context| {
        let mut busy = false;

        for (slot, output) in pending.iter_mut().zip(outputs.iter_mut()) {
            let Some(future) = slot else {
                continue;
            };

            match future.as_mut().poll(context) {
                Poll::Ready(value) => {
                    *output = Some(value);
                    *slot = None;
                }
                Poll::Pending => busy = true,
            }
        }

        if busy { Poll::Pending } else { Poll::Ready(()) }
    })
    .await;

    outputs.into_iter().flatten().collect()
}
