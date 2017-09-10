use super::Receiver;
use super::cell::ReceiverCell;

use futures::Sink;

use std::rc::Weak;

pub struct ActorRef<T: Receiver> {
    pub(super) cell: Weak<ReceiverCell<T>>,
}

impl<T: Receiver> ActorRef<T> {
    pub(super) fn new(cell: Weak<ReceiverCell<T>>) -> ActorRef<T> {
        ActorRef { cell }
    }
}

impl<T: Receiver> ::std::clone::Clone for ActorRef<T> {
    fn clone(&self) -> ActorRef<T> {
        ActorRef::new(self.cell.clone())
    }
}

impl<T: Receiver> ActorRef<T> {
    pub fn send_message(&mut self, message: T::Message) {
        if let Some(cell) = self.cell.upgrade() {
            //TODO: Figure out what to do if this errors out.
            cell.sender.clone().start_send(message);
        }
    }
}
