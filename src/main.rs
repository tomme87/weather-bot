use std::env;
use futures::prelude::*;
use irc::client::prelude::*;
use openweathermap::weather;
use regex::Regex;
use rusqlite::Connection;

#[tokio::main]
async fn main() -> Result<(), failure::Error> {
    let config = Config::load(format!("{}/config.toml", env::current_dir().unwrap().to_str().unwrap())).unwrap();

    println!("hei");
    let api_key = match config.options.get("openweathermap_api_key") {
        Some(k) => String::from(k),
        None => {
            println!("vaaa");
            panic!("missing api key");
        }
    };

    println!("{}", api_key);

    let connection = Connection::open(format!("{}/db.sqlite", env::current_dir().unwrap().to_str().unwrap())).unwrap();
    connection.execute("CREATE TABLE IF NOT EXISTS user (name TEXT UNIQUE, last_location TEXT)", []).unwrap();

    let mut client = Client::from_config(config).await?;
    client.identify()?;

    let mut stream = client.stream()?;

    let re = Regex::new(r"^!v (.+)").unwrap();

    while let Some(message) = stream.next().await.transpose()? {
        print!("{}", message);

        let nick = match message.source_nickname() {
            Some(s) => String::from(s),
            None => String::from("")
        };

        if let Command::PRIVMSG(channel, msg) = message.command {
            if let Some(caps) = re.captures(msg.as_str()) {
                let location = caps.get(1).unwrap().as_str();
                if send_weather(location, &channel, &client, &api_key).await {
                    connection.execute("INSERT INTO user (name, last_location) VALUES(?1,?2) ON CONFLICT(name) DO UPDATE SET last_location=?3", [
                        nick.as_str(),
                        location,
                        location
                    ]).unwrap();
                }
            } else if msg.eq("!v") {
                let mut statement = connection.prepare("SELECT last_location FROM user WHERE name = ?").unwrap();
                let mut rows = statement.query([nick.as_str()]).unwrap();
                if let Some(row) = rows.next()? {
                    let location = row.get::<usize, String>(0).unwrap();
                    send_weather(location.as_str(), &channel, &client, &api_key).await;
                }
            }
        }
    }

    Ok(())
}

async fn send_weather(location: &str, channel: &str, client: &Client, api_key: &str) -> bool {
    match &weather(location, "metric", "no", api_key).await {
        Ok(current) => {
            client.send_privmsg(&channel, format!("{} nå: {}°C, {}", current.name.as_str(), current.main.temp, current.weather[0].description.as_str())).unwrap();
            true
        },
        Err(e) => {
            client.send_privmsg(&channel, e).unwrap();
            false
        }
    }
}
