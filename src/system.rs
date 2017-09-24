use super::{ErrorReceiver, Receiver};
use super::reference::{ActorCell, ActorCellRef, ActorRef};

use tokio_core::reactor::Core;

use std::collections::HashMap;
use std::thread;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct ActorSystem {
    actors: Arc<Mutex<HashMap<String, Box<ActorCellRef>>>>,
}

impl ActorSystem {
    pub fn new() -> ActorSystem {
        ActorSystem {
            actors: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn new_actor<R>(&mut self, id: &str, actor: R) -> ActorBuilder<R>
    where
        R: Receiver + Send + 'static,
    {
        ActorBuilder::new(self.clone(), id.to_owned(), actor)
    }

    pub fn find_actor_ref<T>(&mut self, id: &str) -> Option<ActorRef<T>>
    where
        T: Receiver + 'static,
    {
        if let Ok(ref mut actors) = self.actors.lock() {
            if let Some(actor_cell_ref) = actors.get(id) {
                actor_cell_ref
                    .downcast_ref::<Arc<ActorCell<T>>>()
                    .map(|actor_cell| ActorRef::new(Arc::downgrade(actor_cell)))
            } else {
                None
            }
        } else {
            None
        }
    }
}

pub struct DropErrorReceiver;

impl Receiver for DropErrorReceiver {
    type Message = ();
    type Error = ();

    fn receive(&mut self, _: Self::Message) -> Result<(), ()> {
        Ok(())
    }
}

impl<T> ErrorReceiver<T> for DropErrorReceiver {
    fn receive_error(&mut self, _: T) {}
}

pub struct ActorBuilder<T, E = DropErrorReceiver>
where
    T: Receiver + Send + 'static,
    E: Receiver + ErrorReceiver<T::Error>  + Send + 'static,
{
    actor_system: ActorSystem,
    id: String,
    actor: T,
    error_receiver: Option<ActorRef<E>>,
}

impl<T, E> ActorBuilder<T, E>
where
    T: Receiver + Send + 'static,
    E: Receiver + ErrorReceiver<T::Error> + Send + 'static,
{
    fn new(actor_system: ActorSystem, id: String, actor: T) -> ActorBuilder<T, E> {
        ActorBuilder {
            actor_system,
            id,
            actor,
            error_receiver: None,
        }
    }

    pub fn error_receiver<R>(self, error_receiver: ActorRef<R>) -> ActorBuilder<T, R>
    where
        R: Receiver + ErrorReceiver<T::Error> + 'static,
    {
        ActorBuilder {
            actor_system: self.actor_system,
            actor: self.actor,
            id: self.id,
            error_receiver: Some(error_receiver),
        }
    }

    pub fn spawn(self) -> ActorRef<T> {
        let (ref_sender, ref_receiver) = ::std::sync::mpsc::channel();
        let actor_system = self.actor_system.clone();

        thread::spawn(move || {
            let mut core = Core::new().expect("Unable to create event loop for actor. WutFace");
            let actor_cell = Arc::new(ActorCell::new(self.actor, core.remote()));

            if let Ok(ref mut actors) = actor_system.actors.lock() {
                actors.insert(self.id.clone(), Box::new(actor_cell.clone()));
            }

            let mut actor_ref = ActorRef::new(Arc::downgrade(&actor_cell));
            actor_ref.initialize(actor_system.clone(), core.handle());

            ref_sender
                .send(actor_ref.clone())
                .expect("Failed to return ActorRef. WutFace");

            if let Err(error) = core.run(actor_ref) {
                if let Some(ref mut error_receiver) = self.error_receiver.clone() {
                    error_receiver.send_error(error);
                }
            }

            if let Ok(ref mut actors) = actor_system.actors.lock() {
                actors.remove(&self.id);
            };
        });
 
        ref_receiver.recv().expect("Unable to get remote.")
    }
}
