use super::{ActorRef, ErrorReceiver, Receiver};

use downcast_rs::Downcast;

use futures::{Async, Future, Stream};
use futures::unsync::mpsc;
use futures::unsync::oneshot;

use std::cell::RefCell;
use std::rc::Rc;
use std::error::Error;

pub(super) struct ReceiverCell<T: Receiver> {
    pub actor: RefCell<T>,
    pub sender: mpsc::Sender<T::Message>,
    receiver: RefCell<mpsc::Receiver<T::Message>>,
    error_sender: RefCell<Option<oneshot::Sender<T::Error>>>,
    pub pending_futures: RefCell<Vec<Box<Future<Item = (), Error = Box<Error>>>>>,
}

impl<T: Receiver> ReceiverCell<T> {
    pub(super) fn new(
        actor: T,
        error_sender: Option<oneshot::Sender<T::Error>>,
    ) -> ReceiverCell<T> {
        let (sender, receiver) = mpsc::channel(1024);

        ReceiverCell {
            actor: RefCell::new(actor),
            sender,
            receiver: RefCell::new(receiver),
            error_sender: RefCell::new(error_sender),
            pending_futures: RefCell::new(Vec::new()),
        }
    }
}

//Used to type erase an actor cell.
pub(super) trait ReceiverCellRef: Downcast {
    fn poll(&self);
}

impl_downcast!(ReceiverCellRef);

impl<T: Receiver + 'static> ReceiverCellRef for Rc<ReceiverCell<T>> {
    fn poll(&self) {
        let pending_futures_len = self.pending_futures.borrow().len();

        for index in (0..pending_futures_len).rev() {
            let poll_result = { self.pending_futures.borrow_mut()[index].poll() };

            match poll_result {
                Ok(Async::Ready(_)) => {
                    self.pending_futures.borrow_mut().swap_remove(index);
                }
                Err(_) => {
                    self.pending_futures.borrow_mut().swap_remove(index);
                }
                _ => (),
            }
        }

        match self.receiver.borrow_mut().poll() {
            Ok(Async::Ready(Some(message))) => {
                //Come back and handle errors here.
                if let Err(error) = self.actor.borrow_mut().receive(message) {
                    if let Some(sender) =
                        ::std::mem::replace(&mut *self.error_sender.borrow_mut(), None)
                    {
                        //TODO: Figure out what to do on errors here.
                        sender.send(error);
                    }
                }
            }
            Ok(Async::Ready(None)) => (), // The receiver has been closed at this point?
            Ok(Async::NotReady) => (),
            Err(_) => (), // What do on error?
        }
    }
}

pub(super) struct ErrorReceiverCell<A, R>
where
    A: Receiver,
    R: Receiver + ErrorReceiver<A::Error> + 'static,
{
    receiver: RefCell<oneshot::Receiver<A::Error>>,
    receiver_ref: ActorRef<R>,
}

impl<A, R> ErrorReceiverCell<A, R>
where
    A: Receiver,
    R: Receiver + ErrorReceiver<A::Error> + 'static,
{
    pub fn new(
        receiver: oneshot::Receiver<A::Error>,
        receiver_ref: ActorRef<R>,
    ) -> ErrorReceiverCell<A, R> {
        ErrorReceiverCell {
            receiver: RefCell::new(receiver),
            receiver_ref,
        }
    }
}

pub(super) trait ErrorReceiverCellRef {
    fn poll(&self) -> bool;
}

impl<A, R> ErrorReceiverCellRef for ErrorReceiverCell<A, R>
where
    A: Receiver,
    R: Receiver + ErrorReceiver<A::Error> + 'static,
{
    fn poll(&self) -> bool {
        if let Some(receiver_ref) = self.receiver_ref.cell.upgrade() {
            match self.receiver.borrow_mut().poll() {
                Ok(Async::Ready(error)) => {
                    receiver_ref.actor.borrow_mut().receive_error(error, "");
                    false
                }
                //TODO: Figure out what to do in the other cases.
                _ => true,
            }
        } else {
            true
        }
    }
}
