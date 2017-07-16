use std::mem::replace;
use std::vec::IntoIter;

use futures::{Async, Future, Poll};

use client::ClusterInfo;
use error::Error;
use kv::{FutureSingleMemberKeyValueInfo, KeyValueInfo};
use member::Member;

/// Executes the given closure with each cluster member and short-circuit returns the first
/// successful result. If all members are exhausted without success, the final error is
/// returned.
pub fn first_ok<F>(members: Vec<Member>, callback: F) -> FirstOk<F>
where
    F: Fn(&Member) -> FutureSingleMemberKeyValueInfo,
{
    FirstOk {
        callback,
        current_future: None,
        errors: Vec::with_capacity(members.len()),
        members: members.into_iter(),
    }
}

#[must_use = "futures do nothing unless polled"]
pub struct FirstOk<F>
where
    F: Fn(&Member) -> FutureSingleMemberKeyValueInfo,
{
    callback: F,
    current_future: Option<FutureSingleMemberKeyValueInfo>,
    errors: Vec<Error>,
    members: IntoIter<Member>,
}

impl<F> Future for FirstOk<F>
where
    F: Fn(&Member) -> FutureSingleMemberKeyValueInfo,
{
    type Item = (KeyValueInfo, ClusterInfo);
    type Error = Vec<Error>;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        if let Some(mut current_future) = self.current_future.take() {
            match current_future.poll() {
                Ok(Async::NotReady) => {
                    self.current_future = Some(current_future);

                    Ok(Async::NotReady)
                }
                Ok(Async::Ready(kvi_and_ci)) => Ok(Async::Ready(kvi_and_ci)),
                Err(error) => {
                    self.errors.push(error);

                    self.poll()
                }
            }
        } else {
            match self.members.next() {
                Some(member) => {
                    self.current_future = Some((self.callback)(&member));

                    self.poll()
                }
                None => {
                    let errors = replace(&mut self.errors, vec![]);

                    Err(errors)
                }
            }
        }
    }
}