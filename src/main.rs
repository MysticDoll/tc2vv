use std::{
    collections::HashMap,
    io::Error as IoError,
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio_tungstenite::{accept_async, connect_async, tungstenite::protocol::Message};
use futures_channel::mpsc::{unbounded, UnboundedSender};
use futures_util::{SinkExt, StreamExt};
use tokio::join;
use url::Url;


type Tx = UnboundedSender<Message>;
type PeerMap = Arc<Mutex<HashMap<SocketAddr, Tx>>>;

mod websocket;

#[tokio::main]
async fn main() -> Result<(), String>{
    let server_addr = std::env::var("SERVER_ADDR").unwrap_or("0.0.0.0:8000".to_owned());
    let token = std::env::var("ACCESS_TOKEN").unwrap();
    let channel = std::env::var("TWITCH_CHANNEL").unwrap();
    let url = Url::parse("wss://irc-ws.chat.twitch.tv:443").unwrap();

    let twitch_stream = crate::websocket::connect_websocket(&url).await;

    let (mut write, mut read) = twitch_stream.split();
    crate::websocket::join_chat(&token, &channel, &mut write).await;

    let listener = crate::websocket::create_listener(&server_addr).await;
    let state = PeerMap::new(Mutex::new(HashMap::new()));
    let state_server = state.clone();
    let server_handle = tokio::spawn(async move{
        while let Ok((stream, addr)) = listener.accept().await {
            tokio::spawn(crate::websocket::handle_connection(state_server.clone(), stream, addr));
        }
    });

    let chat_handle = tokio::spawn(crate::websocket::handle_chat(write, read, state.clone()));

    join!(chat_handle, server_handle);

    Err("WebSocket closed".to_owned())
}
