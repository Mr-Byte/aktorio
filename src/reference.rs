use super::{ActorSystem, ErrorReceiver, Receiver};

use futures::{Async, Future, Poll, Sink, Stream};
use futures::sync::mpsc;
use futures::sync::oneshot;

use tokio_core::reactor::{Handle, Remote};

use std::sync::{Arc, Mutex, Weak};

pub(crate) struct ActorCell<T>
where
    T: Receiver,
{
    actor: Mutex<T>,
    mailbox_receiver: Mutex<mpsc::UnboundedReceiver<T::Message>>,
    mailbox_sender: mpsc::UnboundedSender<T::Message>,
    remote: Remote,
}

impl<T> ActorCell<T>
where
    T: Receiver,
{
    pub fn new(actor: T, remote: Remote) -> ActorCell<T> {
        let (mailbox_sender, mailbox_receiver) = mpsc::unbounded();

        ActorCell {
            actor: Mutex::new(actor),
            mailbox_sender,
            mailbox_receiver: Mutex::new(mailbox_receiver),
            remote,
        }
    }
}

pub(crate) trait ActorCellRef: ::downcast_rs::Downcast + Send {}
impl_downcast!(ActorCellRef);

impl<T> ActorCellRef for Arc<ActorCell<T>>
where
    T: Receiver + Send + 'static,
{
}

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
        ActorRef {
            cell: self.cell.clone(),
        }
    }
}

impl<T> ActorRef<T>
where
    T: Receiver + 'static,
{
    pub(crate) fn new(cell: Weak<ActorCell<T>>) -> ActorRef<T> {
        ActorRef { cell }
    }

    pub fn send_message(&mut self, message: T::Message) {
        if let Some(cell) = self.cell.upgrade() {
            let mailbox_sender = cell.mailbox_sender.clone();
            //TODO: Figure out what to do when this errors out. Do we even care if the send fails?
            cell.remote.spawn(move |_| {
                mailbox_sender.send(message).map(|_| ()).map_err(|_| ())
            });
        }
    }

    pub(crate) fn initialize(&mut self, actor_system: ActorSystem, handle: Handle) {
        if let Some(cell) = self.cell.upgrade() {
            if let Ok(ref mut actor) = cell.actor.lock() {
                actor.initialize(actor_system, self.clone(), handle);
            }
        }
    }

    pub(crate) fn send_error<E>(&mut self, error: E)
    where
        E: Send + 'static,
        T: ErrorReceiver<E>,
    {
        if let Some(cell) = self.cell.upgrade() {
            let (error_sender, error_receiver) = oneshot::channel();
            let self_ref = self.clone();

            cell.remote.spawn(move |_| {
                error_receiver
                    .map(move |error| if let Some(cell) = self_ref.cell.upgrade() {
                        if let Ok(ref mut actor) = cell.actor.lock() {
                            actor.receive_error(error);
                        }
                    })
                    .map_err(|_| ())
            });

            //TODO: What do with error?
            error_sender.send(error);
        }
    }

    pub(crate) fn pipe_future<F>(&self, future: F)
    where
        F: Future<Item = T::Message> + Send + 'static,
    {
        if let Some(cell) = self.cell.upgrade() {
            let mailbox_sender = cell.mailbox_sender.clone();

            //TODO: Figure out what to do with the errors.
            cell.remote.spawn(move |_| {
                future
                    .map_err(|_| ())
                    .and_then(|message| mailbox_sender.send(message).map_err(|_| ()))
                    .map(|_| ())
            });
        }
    }

    pub(crate) fn pipe_stream<S>(&self, stream: S)
    where
        S: Stream<Item = T::Message> + Send + 'static,
    {
        if let Some(cell) = self.cell.upgrade() {
            let mailbox_sender = cell.mailbox_sender.clone();

            cell.remote.spawn(move |_| {
                //TODO: Figure out what to do with the errors.
                mailbox_sender
                    .sink_map_err(|_| ())
                    .send_all(stream.map_err(|_| ()))
                    .map(|_| ())
            });
        }
    }
}

impl<T> Future for ActorRef<T>
where
    T: Receiver,
{
    type Item = ();
    type Error = T::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        if let Some(cell) = self.cell.upgrade() {
            if let Ok(ref mut mailbox_receiver) = cell.mailbox_receiver.lock() {
                match mailbox_receiver.poll() {
                    Ok(Async::Ready(Some(message))) => if let Ok(ref mut actor) = cell.actor.lock()
                    {
                        actor.receive(message)?;
                        ::futures::task::current().notify();
                    },
                    _ => (),
                }
            }

            Ok(Async::NotReady)
        } else {
            Ok(Async::Ready(()))
        }
    }
}
