use super::Receiver;
use super::ActorRef;

use futures::{Future, Sink, Stream};

use std::error::Error;

pub trait FutureSender<T: Receiver> {
    fn pipe_to(self, actor_ref: &ActorRef<T>);
}

impl<F, A> FutureSender<A> for F
where
    A: Receiver + 'static,
    F: Future<Item = A::Message, Error = Box<Error>> + 'static,
{
    fn pipe_to(self, actor_ref: &ActorRef<A>) {
        if let Some(cell) = actor_ref.cell.upgrade() {
            let future = {
                let sender = cell.sender.clone();
                self.and_then(|message| {
                    sender
                        .send(message)
                        .map_err(|err| Box::new(err) as Box<Error>)
                }).map(|_| ())
            };

            cell.pending_futures.borrow_mut().push(Box::new(future));
        }
    }
}

pub trait StreamSender<T: Receiver> {
    fn pipe_to(self, actor_ref: &ActorRef<T>);
}

impl<S, A> StreamSender<A> for S
where
    A: Receiver + 'static,
    S: Stream<Item = A::Message, Error = Box<Error>> + 'static,
{
    fn pipe_to(self, actor_ref: &ActorRef<A>) {
        if let Some(cell) = actor_ref.cell.upgrade() {
            let future = {
                let sender = cell.sender.clone();
                self.forward(sender).map(|_| ())
            };

            cell.pending_futures.borrow_mut().push(Box::new(future));
        }
    }
}
