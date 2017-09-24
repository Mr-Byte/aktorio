use super::Receiver;
use super::ActorRef;

use futures::{Future, Stream};

pub trait FutureSender<T: Receiver> {
    fn pipe_to(self, actor_ref: &ActorRef<T>);
}

impl<F, A> FutureSender<A> for F
where
    A: Receiver + 'static,
    F: Future<Item = A::Message> + Send + 'static,
{
    fn pipe_to(self, actor_ref: &ActorRef<A>) {
        actor_ref.pipe_future(self);
    }
}

pub trait StreamSender<T: Receiver> {
    fn pipe_to(self, actor_ref: &ActorRef<T>);
}

impl<S, A> StreamSender<A> for S
where
    A: Receiver + 'static,
    S: Stream<Item = A::Message> + Send + 'static,
{
    fn pipe_to(self, actor_ref: &ActorRef<A>) {
        actor_ref.pipe_stream(self);
    }
}
