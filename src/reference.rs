use Receiver;
use cell::{ActorCell, ActorCellWeakRef};

use futures::{Future, Sink, Stream};

use tokio_core::reactor::Remote;

use std::sync::Weak;

pub struct ActorRef<T>
where
    T: Receiver + 'static,
{
    cell: Weak<ActorCell<T>>,
}

impl<T> ::std::clone::Clone for ActorRef<T>
where
    T: Receiver + 'static,
{
    fn clone(&self) -> ActorRef<T> {
        ActorRef { cell: self.cell.clone() }
    }
}

impl<T> ActorRef<T>
where
    T: Receiver + Send + 'static,
{
    pub(crate) fn new(cell: Weak<ActorCell<T>>) -> ActorRef<T> {
        ActorRef { cell: cell }
    }

    pub fn id(&self) -> Result<String, String> {
        self.cell.id()
    }

    pub fn remote(&self) -> Result<Remote, String> {
        self.cell.remote()
    }

    pub fn send_message(&self, message: T::Message) -> Result<(), String> {
        let mailbox_sender = self.cell.mailbox_sender()?;
        //TODO: Figure out what to do when this errors out. Do we even care if the send fails?
        self.cell.remote()?.spawn(move |_| {
            mailbox_sender.send(message).map(|_| ()).map_err(|_| ())
        });

        Ok(())
    }

    pub(crate) fn pipe_future<F>(&self, future: F) -> Result<(), String>
    where
        F: Future<Item = T::Message> + Send + 'static,
    {
        let mailbox_sender = self.cell.mailbox_sender()?;

        //TODO: Figure out what to do with the errors.
        self.cell.remote()?.spawn(move |_| {
            future
                .map_err(|_| ())
                .and_then(|message| mailbox_sender.send(message).map_err(|_| ()))
                .map(|_| ())
        });

        Ok(())
    }

    pub(crate) fn pipe_stream<S>(&self, stream: S) -> Result<(), String>
    where
        S: Stream<Item = T::Message> + Send + 'static,
    {
        let mailbox_sender = self.cell.mailbox_sender()?;

        self.cell.remote()?.spawn(move |_| {
            //TODO: Figure out what to do with the errors.
            mailbox_sender
                .sink_map_err(|_| ())
                .send_all(stream.map_err(|_| ()))
                .map(|_| ())
        });

        Ok(())
    }
}
