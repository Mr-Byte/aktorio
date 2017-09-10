use super::{ErrorReceiver, Receiver};
use super::reference::ActorRef;
use super::cell::{ErrorReceiverCell, ErrorReceiverCellRef, ReceiverCell, ReceiverCellRef};

use futures::{Async, Future, Poll};
use futures::task;
use futures::unsync::oneshot;

use std::rc::{Rc, Weak};
use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;

pub struct ActorSystem {
    context: Rc<ActorContext>,
    polling_actors: Vec<Weak<ReceiverCellRef>>,
}

impl ActorSystem {
    pub fn new() -> ActorSystem {
        ActorSystem {
            context: Rc::new(ActorContext::new()),
            polling_actors: Vec::new(),
        }
    }

    pub fn spawn<T>(&mut self, id: &str, receiver: T) -> ActorRef<T>
    where
        T: Receiver + 'static,
    {
        spawn(&self.context, id.to_owned(), receiver)
    }

    pub fn find_actor_ref<T>(&mut self, id: &str) -> Option<ActorRef<T>>
    where
        T: Receiver + 'static,
    {
        find_actor_ref(&self.context, id)
    }
}

impl Future for ActorSystem {
    type Item = ();
    type Error = Box<Error>;

    fn poll(&mut self) -> Poll<(), Box<Error>> {
        self.polling_actors.clear();
        self.polling_actors.extend(
            self.context
                .actors
                .borrow_mut()
                .values()
                .map(|actor| Rc::downgrade(actor)),
        );

        for actor in &self.polling_actors {
            if let Some(actor) = actor.upgrade() {
                actor.poll();
            }
        }

        self.context
            .error_receivers
            .borrow_mut()
            .retain(|error_receiver| error_receiver.poll());

        task::current().notify();

        Ok(Async::NotReady)
    }
}

pub struct ActorContext {
    actors: RefCell<HashMap<String, Rc<ReceiverCellRef>>>,
    error_receivers: RefCell<Vec<Box<ErrorReceiverCellRef>>>,
}

impl ActorContext {
    fn new() -> ActorContext {
        ActorContext {
            actors: RefCell::new(HashMap::new()),
            error_receivers: RefCell::new(Vec::new()),
        }
    }
}

pub struct ActorContextRef {
    context: Weak<ActorContext>,
}

impl ActorContextRef {
    fn new(context: &Rc<ActorContext>) -> ActorContextRef {
        ActorContextRef {
            context: Rc::downgrade(context),
        }
    }

    pub fn new_actor<T>(&mut self, id: &str, actor: T) -> ReceiverBuilder<T>
    where
        T: Receiver + 'static,
    {
        ReceiverBuilder::new(
            self.context
                .upgrade()
                .expect("Context is no longer associated with an ActorSystem"),
            id,
            actor,
        )
    }

    pub fn find_actor_ref<T>(&mut self, id: &str) -> Option<ActorRef<T>>
    where
        T: Receiver + 'static,
    {
        find_actor_ref(
            &self.context
                .upgrade()
                .expect("Context is no longer associated with an ActorSystem"),
            id,
        )
    }
}

#[must_use = "Unused ReceiverBuilder, use spawn to spawn the actor."]
pub struct ReceiverBuilder<A>
where
    A: Receiver + 'static,
{
    context: Rc<ActorContext>,
    actor: A,
    id: String,
}

impl<A> ReceiverBuilder<A>
where
    A: Receiver + 'static,
{
    fn new(context: Rc<ActorContext>, id: &str, actor: A) -> ReceiverBuilder<A> {
        ReceiverBuilder {
            context,
            actor,
            id: id.to_owned(),
        }
    }

    pub fn error_receiver<R>(self, error_receiver: ActorRef<R>) -> ErrorReceiverBuilder<A, R>
    where
        R: Receiver + ErrorReceiver<A::Error> + 'static,
    {
        ErrorReceiverBuilder {
            inner: self,
            error_receiver,
        }
    }

    pub fn spawn(self) -> ActorRef<A> {
        spawn(&self.context, self.id, self.actor)
    }
}

#[must_use = "Unused ActorBuilder, use spawn to spawn the actor."]
pub struct ErrorReceiverBuilder<A, R>
where
    A: Receiver + 'static,
    R: Receiver + ErrorReceiver<A::Error> + 'static,
{
    inner: ReceiverBuilder<A>,
    error_receiver: ActorRef<R>,
}

impl<A, R> ErrorReceiverBuilder<A, R>
where
    A: Receiver + 'static,
    R: Receiver + ErrorReceiver<A::Error> + 'static,
{
    pub fn spawn(self) -> ActorRef<A> {
        spawn_with_error_receiver(
            &self.inner.context,
            self.inner.id,
            self.inner.actor,
            self.error_receiver,
        )
    }
}

#[inline]
fn spawn<A>(context: &Rc<ActorContext>, id: String, actor: A) -> ActorRef<A>
where
    A: Receiver + 'static,
{
    let actor_cell = initalize_receiver_cell(context, actor, None);

    insert_receiver_cell(context, id, &actor_cell);
    ActorRef::new(Rc::downgrade(&actor_cell))
}

#[inline]
fn spawn_with_error_receiver<A, R>(
    context: &Rc<ActorContext>,
    id: String,
    actor: A,
    error_receiver_ref: ActorRef<R>,
) -> ActorRef<A>
where
    A: Receiver + 'static,
    R: Receiver + ErrorReceiver<A::Error> + 'static,
{
    let (error_sender, error_receiver) = oneshot::channel();
    let actor_cell = initalize_receiver_cell(context, actor, Some(error_sender));

    {
        let error_receiver_cell = Box::new(ErrorReceiverCell::<A, R>::new(
            error_receiver,
            error_receiver_ref,
        )) as Box<ErrorReceiverCellRef>;

        context
            .error_receivers
            .borrow_mut()
            .push(error_receiver_cell);
    }

    insert_receiver_cell(context, id, &actor_cell);

    ActorRef::new(Rc::downgrade(&actor_cell))
}

#[inline]
fn initalize_receiver_cell<A>(
    context: &Rc<ActorContext>,
    actor: A,
    error_sender: Option<oneshot::Sender<A::Error>>,
) -> Rc<ReceiverCell<A>>
where
    A: Receiver + 'static,
{
    let actor_cell = Rc::new(ReceiverCell::new(actor, error_sender));
    actor_cell.actor.borrow_mut().initialize(
        ActorContextRef::new(context),
        ActorRef::new(Rc::downgrade(&actor_cell)),
    );

    actor_cell
}

#[inline]
fn insert_receiver_cell<A>(context: &Rc<ActorContext>, id: String, actor_cell: &Rc<ReceiverCell<A>>)
where
    A: Receiver + 'static,
{
    context
        .actors
        .borrow_mut()
        .insert(id, Rc::new(actor_cell.clone()) as Rc<ReceiverCellRef>);
}

#[inline]
fn find_actor_ref<T>(context: &Rc<ActorContext>, id: &str) -> Option<ActorRef<T>>
where
    T: Receiver + 'static,
{
    context.actors.borrow().get(id).and_then(|cell_ref| {
        cell_ref
            .downcast_ref::<Rc<ReceiverCell<T>>>()
            .map(|actor_cell| ActorRef::new(Rc::downgrade(&actor_cell)))
    })
}
