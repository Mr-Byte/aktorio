use super::Receiver;

use futures::sync::mpsc;
use tokio_core::reactor::Remote;

use std::sync::{LockResult, Mutex, MutexGuard, Weak};

pub struct ActorCell<T>
where
    T: Receiver,
{
    id: String,
    mailbox_sender: mpsc::UnboundedSender<T::Message>,
    remote: Remote,
    actor: Mutex<T>,
}

impl<T> ActorCell<T>
where
    T: Receiver,
{
    pub fn new(
        actor: T,
        id: &str,
        mailbox_sender: mpsc::UnboundedSender<T::Message>,
        remote: Remote,
    ) -> ActorCell<T> {
        ActorCell {
            actor: Mutex::new(actor),
            id: id.to_owned(),
            mailbox_sender,
            remote,
        }
    }

    #[inline]
    pub fn id(&self) -> String {
        self.id.clone()
    }

    #[inline]
    pub fn remote(&self) -> Remote {
        self.remote.clone()
    }

    #[inline]
    pub fn actor(&self) -> LockResult<MutexGuard<T>> {
        self.actor.lock()
    }

    #[inline]
    pub fn mailbox_sender(&self) -> mpsc::UnboundedSender<T::Message> {
        self.mailbox_sender.clone()
    }
}

pub trait ActorCellWeakRef<T>
where
    T: Receiver,
{
    fn try_get<F, R>(&self, property_selector: F) -> Result<R, String>
    where
        F: FnOnce(&ActorCell<T>) -> R,
        R: Send;

    #[inline]
    fn id(&self) -> Result<String, String> {
        self.try_get(|cell| cell.id())
    }

    #[inline]
    fn remote(&self) -> Result<Remote, String> {
        self.try_get(|cell| cell.remote())
    }

    #[inline]
    fn mailbox_sender(&self) -> Result<mpsc::UnboundedSender<T::Message>, String> {
        self.try_get(|cell| cell.mailbox_sender())
    }
}

impl<T> ActorCellWeakRef<T> for Weak<ActorCell<T>>
where
    T: Receiver,
{
    fn try_get<F, R>(&self, property_selector: F) -> Result<R, String>
    where
        F: FnOnce(&ActorCell<T>) -> R,
        R: Send,
    {
        self.upgrade().map(|cell| property_selector(&*cell)).ok_or(
            "Failed to get property.".to_owned(),
        )
    }
}
