use {ErrorReceiver, Receiver};
use reference::ActorRef;
use context::{ActorContext, ActorContextRef, ActorFuture};
use Context;

use tokio_core::reactor::{Core, Handle};

use std::collections::HashMap;
use std::thread;
use std::cell::RefCell;
use std::sync::{Arc, RwLock, Weak};

thread_local! {
    static CORE: RefCell<Core> = RefCell::new(Core::new().expect("Unable to create core for thread."));
    pub(crate) static HANDLE: Handle = CORE.with(|core| core.borrow().handle());
}

pub struct ActorSystemContext {
    actors: HashMap<String, Box<ActorContextRef>>,
}

impl ActorSystemContext {
    fn new() -> ActorSystemContext {
        ActorSystemContext { actors: HashMap::new() }
    }

    #[inline]
    pub fn find_actor_ref<T>(&self, id: &str) -> Option<ActorRef<T>>
    where
        T: Receiver + Send + 'static,
    {
        self.find_actor_context(id).map(
            |context| context.actor_ref(),
        )
    }

    #[inline]
    fn find_actor_context<R>(&self, id: &str) -> Option<Arc<ActorContext<R>>>
    where
        R: Receiver + 'static,
    {
        self.actors.get(id).and_then(|context_ref| {
            context_ref.downcast_ref::<Arc<ActorContext<R>>>().map(
                |context| context.clone(),
            )
        })
    }
}

pub struct ActorSystem {
    context: Arc<RwLock<ActorSystemContext>>,
}

impl ActorSystem {
    pub fn new() -> ActorSystem {
        ActorSystem { context: Arc::new(RwLock::new(ActorSystemContext::new())) }
    }

    pub fn new_actor<R>(&self, id: &str, actor: R) -> ActorBuilder<R>
    where
        R: Receiver + 'static,
    {
        ActorBuilder::new(Arc::downgrade(&self.context), id.to_owned(), actor)
    }

    pub fn find_actor_ref<T>(&self, id: &str) -> Option<ActorRef<T>>
    where
        T: Receiver + Send + 'static,
    {
        self.context.read().ok().and_then(
            |context| context.find_actor_ref(id),
        )
    }

    pub fn run(&self) {
        loop {
            if let Ok(ref context) = self.context.read() {
                if context.actors.len() == 0 {
                    break;
                }
            }
        }
    }
}

pub struct DropErrorReceiver;

impl Receiver for DropErrorReceiver {
    type Message = ();
    type Error = ();

    fn receive(&mut self, _: Self::Message, _: &Context<Self>) -> Result<(), ()> {
        Ok(())
    }
}

impl<T> ErrorReceiver<T> for DropErrorReceiver {
    fn receive_error(&mut self, _: T) {}
}

pub struct ActorBuilder<T, E = DropErrorReceiver>
where
    T: Receiver + 'static,
    E: Receiver + ErrorReceiver<T::Error> + 'static,
{
    context: Weak<RwLock<ActorSystemContext>>,
    id: String,
    actor: T,
    error_receiver: Option<ActorRef<E>>,
}

impl<T, E> ActorBuilder<T, E>
where
    T: Receiver + 'static,
    E: Receiver + ErrorReceiver<T::Error> + 'static,
{
    pub(crate) fn new(
        context: Weak<RwLock<ActorSystemContext>>,
        id: String,
        actor: T,
    ) -> ActorBuilder<T, E> {
        ActorBuilder {
            context,
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
            context: self.context,
            actor: self.actor,
            id: self.id,
            error_receiver: Some(error_receiver),
        }
    }

    pub fn spawn(self) -> ActorRef<T> {
        let (ref_sender, ref_receiver) = ::std::sync::mpsc::channel();
        let context = self.context.clone();

        thread::spawn(move || {
            CORE.with(move |core| {

                let actor_context = Arc::new(ActorContext::new(&self.id, self.actor, context.clone()));
                let actor_ref = actor_context.actor_ref().clone();

                if let Some(context) = context.upgrade() {
                    if let Ok(ref mut context) = context.write() {
                        context.actors.insert(self.id.clone(), Box::new(actor_context.clone()));
                    }

                    ref_sender.send(actor_ref).expect(
                        "Failed to return ActorRef. WutFace",
                    );

                    let actor_future = ActorFuture::new(actor_context.clone());

                    if let Err(error) = core.borrow_mut().run(actor_future) {
                        if let Some(error_receiver_ref) = self.error_receiver {
                            if let Ok(id) = error_receiver_ref.id() {
                                if let Ok(context) = context.read() {
                                    if let Some(error_receiver) =
                                        context.find_actor_context::<E>(&id)
                                    {
                                        error_receiver.send_error(error);
                                    }
                                }
                            }
                        }
                    }

                    // Turn the event loop one last time to poll any futures that may have been queued on the event loop, one last time.
                    // I'm not really happy with this solution and may investigate a better way to handle this.  Without this, there were panics
                    // when the actor in question errored out, but there were still respondable messages responses queued up.
                    core.borrow_mut().turn(None);

                    if let Ok(ref mut context) = context.write() {
                        context.actors.remove(&self.id);
                    };
                }
            });
        });

        ref_receiver.recv().expect("Unable to get remote.")
    }
}
