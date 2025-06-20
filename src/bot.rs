use serenity::async_trait;
use serenity::model::gateway::Ready;
use serenity::model::prelude::*;
use serenity::prelude::*;
use serenity::framework::standard::{
    macros::{command, group},
    CommandResult, StandardFramework,
};
use crate::music;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[group]
#[commands(goon, queue, skip)]
struct General;

pub fn run() {
    let token = std::fs::read_to_string("../secret.secret").expect("token file");
    let framework = StandardFramework::new()
        .configure(|c| c.prefix("!"))
        .group(&GENERAL_GROUP);

    let mut client = serenity::Client::builder(token, GatewayIntents::all())
        .event_handler(Handler)
        .framework(framework)
        .register_songbird()
        .expect("Error creating client");

    if let Err(why) = client.start() {
        println!("Client error: {:?}", why);
    }
}

#[command]
async fn goon(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let query = args.rest();
    if query.is_empty() {
        msg.reply(ctx, "Please provide a search query.").await?;
        return Ok(());
    }
    music::play_youtube(ctx, msg, query).await?;
    Ok(())
}

#[command]
async fn queue(ctx: &Context, msg: &Message) -> CommandResult {
    crate::music::show_queue(ctx, msg).await?;
    Ok(())
}

#[command]
async fn skip(ctx: &Context, msg: &Message) -> CommandResult {
    crate::music::skip(ctx, msg).await?;
    Ok(())
}