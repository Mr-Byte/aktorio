use futures::sync::mpsc;
use futures::{Async, Future, Poll, Stream};

use std::sync::{Arc, Mutex, RwLock, Weak};

use {ActorRef, ErrorReceiver, Receiver};
use cell::ActorCell;
use system::ActorSystemContext;

pub struct ActorContext<T>
where
    T: Receiver + Send + 'static,
{
    mailbox_receiver: Mutex<mpsc::UnboundedReceiver<T::Message>>,
    context: Weak<RwLock<ActorSystemContext>>,
    cell: Arc<ActorCell<T>>,
}

impl<T> ActorContext<T>
where
    T: Receiver + Send + 'static,
{
    pub fn new(id: &str, actor: T, context: Weak<RwLock<ActorSystemContext>>) -> ActorContext<T> {
        super::system::HANDLE.with(|handle| {
            let (mailbox_sender, mailbox_receiver) = mpsc::unbounded();
            let cell = ActorCell::new(actor, id, mailbox_sender, handle.remote().clone());

            ActorContext {
                mailbox_receiver: Mutex::new(mailbox_receiver),
                context,
                cell: Arc::new(cell),
            }
        })
    }

    pub fn actor_ref(&self) -> ActorRef<T> {
        ActorRef::new(Arc::downgrade(&self.cell))
    }

    pub fn send_error<E>(&self, error: E)
    where
        T: ErrorReceiver<E>,
        E: Send + 'static,
    {
        let cell = self.cell.clone();

        self.cell.remote().spawn(move |_| {
            if let Ok(ref mut actor) = cell.actor() {
                actor.receive_error(error);
            }

            Ok(())
        });
    }
}

pub trait ActorContextRef: ::downcast_rs::Downcast + Send + Sync {}

impl_downcast!(ActorContextRef);

impl<T> ActorContextRef for Arc<ActorContext<T>>
where
    T: Receiver + Send + 'static,
{
}

pub struct ActorFuture<T>
where
    T: Receiver + 'static,
{
    context: Arc<ActorContext<T>>,
}

impl<T> ActorFuture<T>
where
    T: Receiver + 'static,
{
    pub fn new(context: Arc<ActorContext<T>>) -> ActorFuture<T> {
        ActorFuture { context }
    }
}

impl<T> Future for ActorFuture<T>
where
    T: Receiver,
{
    type Item = ();
    type Error = T::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        if let Ok(ref mut mailbox_receiver) = self.context.mailbox_receiver.lock() {
            if let Ok(Async::Ready(Some(message))) = mailbox_receiver.poll() {
                if let Ok(ref mut actor) = self.context.cell.actor() {

                    ::system::HANDLE.with(|handle| {
                        let mut context = ::Context::new(handle.clone(), ActorRef::new(Arc::downgrade(&self.context.cell)), self.context.context.clone());
                        Ok(actor.receive(message, &mut context)?)
                    })?
                }
            }

            ::futures::task::current().notify();
            Ok(Async::NotReady)
        } else {
            Ok(Async::Ready(()))
        }
    }
}
