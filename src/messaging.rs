use super::Receiver;
use super::ActorRef;

use futures::{Future, Poll, Stream};
use futures::sync::oneshot;

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

pub struct RespondableMessage<M, R>
where
    M: Send + 'static,
    R: Send + 'static,
{
    message: M,
    sender: oneshot::Sender<R>,
}

pub struct Response<R>
where
    R: Send + 'static,
{
    receiver: oneshot::Receiver<R>,
}

impl<R> Response<R>
where
    R: Send + 'static,
{
    fn new(receiver: oneshot::Receiver<R>) -> Response<R> {
        Response { receiver }
    }
}

impl<R> Future for Response<R>
where
    R: Send + 'static,
{
    type Item = R;
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        Ok(self.receiver.poll().map_err(|_| ())?)
    }
}

#[must_use = "RespondableMessage must have a response sent through the respond() method."]
impl<M, R> RespondableMessage<M, R>
where
    M: Send + 'static,
    R: Send + 'static,
{
    pub fn new(message: M) -> (Response<R>, RespondableMessage<M, R>) {
        let (sender, receiver) = oneshot::channel();
        let response = Response::new(receiver);
        let responder = RespondableMessage { message, sender };

        (response, responder)
    }

    pub fn message(&self) -> &M {
        &self.message
    }

    pub fn respond(self, response: R) {
        super::system::HANDLE.with(move |handle| {
            handle.spawn_fn(move || self.sender.send(response).map_err(|_| ()));
        });
    }
}
