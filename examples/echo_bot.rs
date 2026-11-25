//! An echo bot: prints its address, accepts everyone who asks, and says back what it hears.

use simplex_client::{Client, Event};

fn main() -> simplex_client::Result<()> {
    let client = Client::builder("./bot-data")
        .name("Echo Bot")
        .key("store passphrase")
        .start()?;
    println!("my address: {}", client.create_address()?);
    for event in client.events() {
        match event {
            Event::Request(request) => request.accept()?,
            Event::FriendAdd(friend) => {
                friend.send("Hi! I say back what you write.")?;
            }
            Event::Message(msg) if !msg.mine() && !msg.text().is_empty() => {
                msg.reply(msg.text())?;
            }
            _ => {}
        }
    }
    Ok(())
}
