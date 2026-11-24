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
            Event::Messages(msgs) => {
                for m in msgs.iter().filter(|m| !m.mine() && !m.text().is_empty()) {
                    client.reply(m, m.text())?;
                }
            }
            Event::Chats { requests, .. } => {
                for r in requests {
                    client.accept(r.id)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}
