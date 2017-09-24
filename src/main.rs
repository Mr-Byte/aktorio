extern crate aktorio;
extern crate futures;
extern crate tokio_core;

use aktorio::{ActorRef, ActorSystem, ErrorReceiver, Receiver, StreamSender};
use futures::stream;

pub fn main() {
    let mut actor_system = ActorSystem::new();
    let mut actor_ref = actor_system.new_actor("butts", TestActor::new()).spawn();

    let messages = stream::iter_ok::<_, ()>(vec![
        "Hello, Mercury!".to_owned(),
        "Hello, Venus!".to_owned(),
        "Hello, Earth!".to_owned(),
        "Hello, Mars!".to_owned(),
        "Hello, Jupiter!".to_owned(),
        "Hello, Saturn!".to_owned(),
        "Hello, Uranus!".to_owned(),
        "Hello, Neptune!".to_owned(),
        "Hello, Pluto!".to_owned(),
        "Hello, Oort Cloud!".to_owned(),
    ]);

    messages.pipe_to(&mut actor_ref);

    ::std::thread::sleep(::std::time::Duration::from_secs(10));
}

struct TestActor {
    other_actor: Option<ActorRef<OtherActor>>,
}

impl TestActor {
    fn new() -> TestActor {
        TestActor { other_actor: None }
    }
}

impl Receiver for TestActor {
    type Message = String;
    type Error = ();

    fn initialize(
        &mut self,
        mut actor_system: ActorSystem,
        self_ref: ActorRef<Self>,
        _: ::tokio_core::reactor::Handle,
    ) {
        self.other_actor = Some(
            actor_system
                .new_actor("other", OtherActor)
                .error_receiver(self_ref)
                .spawn(),
        );
    }

    fn receive(&mut self, message: Self::Message) -> Result<(), ()> {
        println!("TestActor: {}", message);
        self.other_actor.as_mut().unwrap().send_message(message);

        Ok(())
    }
}

impl ErrorReceiver<String> for TestActor {
    fn receive_error(&mut self, error: String) {
        println!("Received error: {}", error);
    }
}

struct OtherActor;

impl Receiver for OtherActor {
    type Message = String;
    type Error = String;

    fn receive(&mut self, message: Self::Message) -> Result<(), String> {
        println!("OtherActor: {}", message);

        Ok(())
    }
}
