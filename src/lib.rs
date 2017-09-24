#[macro_use]
extern crate downcast_rs;
extern crate futures;
extern crate tokio_core;

pub mod system;
pub mod reference;
pub mod messaging;

pub use self::system::ActorSystem;
pub use self::reference::ActorRef;
pub use self::messaging::{FutureSender, StreamSender};

pub trait Receiver
where
    Self: Send,
{
    type Message: Send;
    type Error: Send;

    fn initialize(
        &mut self,
        _actor_system: ActorSystem,
        _self_ref: ActorRef<Self>,
        _handle: ::tokio_core::reactor::Handle,
    ) where
        Self: Sized,
    {
    }

    fn receive(&mut self, message: Self::Message) -> Result<(), Self::Error>;
}

pub trait ErrorReceiver<T> {
    fn receive_error(&mut self, error: T);
}

impl<T> Receiver for T
where
    T: ::futures::Sink + Send,
    T::SinkItem: Send,
    T::SinkError: Send,
{
    type Message = <Self as ::futures::Sink>::SinkItem;
    type Error = <Self as ::futures::Sink>::SinkError;

    fn receive(&mut self, message: Self::Message) -> Result<(), Self::Error> {
        self.start_send(message)?;
        self.poll_complete()?;

        Ok(())
    }
}
