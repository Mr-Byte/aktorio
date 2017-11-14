extern crate aktorio;
extern crate futures;
extern crate tokio_core;

use aktorio::{ActorSystem, Context, Receiver};

pub fn main() {
    let actor_system = ActorSystem::new();
    let first_actor = actor_system.new_actor("first", FirstActor).spawn();
    first_actor.send_message("Hello, world!".to_owned());

    actor_system.run();
}

struct FirstActor;

impl Receiver for FirstActor {
    type Message = String;
    type Error = String;

    fn receive(&mut self, message: String, context: &Context<Self>) -> Result<(), String> {
        println!("FirstActor: {}", message);

        let second_actor = context.new_actor("other", SecondActor).spawn();
        second_actor.send_message(message.to_lowercase())?;

        Ok(())
    }
}

struct SecondActor;

impl Receiver for SecondActor {
    type Message = String;
    type Error = ();

    fn receive(&mut self, message: Self::Message, _: &Context<Self>) -> Result<(), ()> {
        println!("SecondActor: {}", message);

        Ok(())
    }
}
