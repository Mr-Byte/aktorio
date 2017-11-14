#[macro_use]
extern crate downcast_rs;
extern crate futures;
extern crate tokio_core;

mod cell;
mod context;
mod system;
mod reference;
mod messaging;

pub use self::system::ActorSystem;
pub use self::reference::ActorRef;
pub use self::messaging::{FutureSender, StreamSender, RespondableMessage, Response};

use tokio_core::reactor::Handle;

use std::sync::{Weak, RwLock};

pub trait Receiver: Send {
    type Message: Send;
    type Error: Send;

    fn receive(&mut self, message: Self::Message, context: &Context<Self>) -> Result<(), Self::Error> where Self: Sized;
}

pub trait ErrorReceiver<T> {
    fn receive_error(&mut self, error: T);
}

//TODO: Figure out some way to use thread local storage to schedule the send as a message on the event loop, so it gets polled to completion?
impl<T> Receiver for T
where
    T: ::futures::Sink + Send,
    T::SinkItem: Send,
    T::SinkError: Send,
{
    type Message = <Self as ::futures::Sink>::SinkItem;
    type Error = <Self as ::futures::Sink>::SinkError;

    fn receive(&mut self, message: Self::Message, _: &Context<Self>) -> Result<(), Self::Error> {
        self.start_send(message)?;
        self.poll_complete()?;

        Ok(())
    }
}

pub struct Context<T>
where
    T: Receiver + 'static,
{
    handle: Handle,
    self_ref: ActorRef<T>,
    context: Weak<RwLock<system::ActorSystemContext>>,
}

impl<T> Context<T>
where
    T: Receiver + 'static,
{
    pub(crate) fn new(handle: Handle, self_ref: ActorRef<T>, context: Weak<RwLock<system::ActorSystemContext>>) -> Context<T> {
        Context {
            handle,
            self_ref,
            context
        }
    }

    pub fn handle(&self) -> Handle {
        self.handle.clone()
    }

    pub fn self_ref(&self) -> ActorRef<T> {
        self.self_ref.clone()
    }

    pub fn new_actor<R>(&self, id: &str, actor: R) -> system::ActorBuilder<R>
    where
        R: Receiver + 'static,
    {
        system::ActorBuilder::new(self.context.clone(), id.to_owned(), actor)
    }

    pub fn find_actor_ref<R>(&self, id: &str) -> Option<ActorRef<R>>
    where
        R: Receiver + Send + 'static,
    {
        self.context.upgrade().and_then(|context| {
            context.read().ok().and_then(
                |context| context.find_actor_ref(id),
            )
        })
    }
}
