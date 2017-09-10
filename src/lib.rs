#[macro_use]
extern crate downcast_rs;
extern crate futures;

pub mod system;
pub mod reference;
pub mod messaging;
mod cell;

pub use self::system::{ActorContextRef, ActorSystem};
pub use self::reference::ActorRef;
pub use self::messaging::{FutureSender, StreamSender};

pub trait Receiver {
    type Message;
    type Error;

    fn initialize(&mut self, _context: ActorContextRef, _self_ref: ActorRef<Self>)
    where
        Self: Sized,
    {
    }

    fn receive(&mut self, message: Self::Message) -> Result<(), Self::Error>;
}

pub trait ErrorReceiver<T> {
    fn receive_error(&mut self, error: T, actor_id: &str);
}

impl<T> Receiver for T
where
    T: ::futures::Sink,
{
    type Message = <Self as ::futures::Sink>::SinkItem;
    type Error = <Self as ::futures::Sink>::SinkError;

    fn receive(&mut self, message: Self::Message) -> Result<(), Self::Error> {
        self.start_send(message)?;
        self.poll_complete()?;

        Ok(())
    }
}
