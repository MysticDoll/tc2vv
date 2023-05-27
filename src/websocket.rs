use std::net::SocketAddr;
use tokio_tungstenite::{accept_async, connect_async, tungstenite::protocol::Message};
use futures_util::{pin_mut, SinkExt, StreamExt};
use futures_channel::mpsc::{unbounded, UnboundedSender};
use tokio::net::{TcpListener, TcpStream};
use url::Url;

pub(crate)async fn connect_websocket(url: &Url) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>> {
    let (stream, _) = connect_async(url).await.unwrap();
    stream
}

pub(crate) async fn join_chat(token: &str, channel: &str, write: &mut futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, Message>) {
    if let Err(_e) = write.send(
        Message::Text(
            format!(
                "PASS oauth:{}",
                token
            )
        )
    ).await {
        eprintln!("websocket auth error");
    };

    if let Err(_e) = write.send(
        Message::Text(
            format!("NICK {}", channel)
        )
    ).await {
        eprintln!("send nickname error");
    };

    if let Err(_e) = write.send(
        Message::Text(
            format!("JOIN #{}", channel)
        )
    ).await {
        eprintln!("channel join error");
    };

}

pub(crate) async fn handle_chat(
    mut write: futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, Message>,
    mut read: futures_util::stream::SplitStream<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>,
    peer_map: crate::PeerMap
) {
    let voicevox_url = format!(
        "{}/audio_query",
        std::env::var("VOICEVOX_URL").unwrap()
    );

    let chat_handle = tokio::spawn(async move {
        let ping_rex = regex::Regex::new(r"PING\s:tmi\.twitch\.tv").unwrap();
        let msg_rex = regex::Regex::new(
            r":.+\.tmi\.twitch\.tv\sPRIVMSG\s#.+\s:(?P<message>.+)\r\n"
        ).unwrap();
        while let Some(Ok(Message::Text(msg))) = read.next().await {
            if ping_rex.is_match(&msg) {
                if let Err(_e) = write.send(Message::Text("PONG :tmi.twitch.tv".to_owned())).await {
                    eprintln!("pong failed");
                } else {
                    println!("pong succeed");
                };
            } else {
                println!("message: {}", msg);
                if let Some(caps) = msg_rex.captures(&msg) {
                    if let Some(raw_msg) = caps.name("message") {
                        let message = raw_msg.as_str();
                        if let Ok(res) = reqwest::Client::new().post(&voicevox_url)
                            .query(&[
                                ("speaker", "1"),
                                ("text", message)
                            ])
                            .send()
                            .await {
                                let query = res.text().await.unwrap_or("voicevox request error".to_owned());
                                let peers = peer_map.lock().unwrap();

                                for (_, sink) in peers.iter() {
                                    if let Err(e) = sink.unbounded_send(Message::Text(query.clone())) {
                                        eprintln!("error occured when sending message to client: {:?}", e);
                                    };
                                }
                            }
                    };
                }
            }
        }
    });

    chat_handle.await;
}

pub(crate) async fn create_listener(server_addr: &str) -> TcpListener {
    let listener = TcpListener::bind(server_addr).await.unwrap();
    println!("server listening on: {}", server_addr);

    listener
}

pub(crate) async fn handle_connection(peer_map: crate::PeerMap, raw_stream: TcpStream, addr: SocketAddr) {
    let ws_stream = accept_async(raw_stream).await.unwrap();

    let (tx, rx) = unbounded();
    let ping_tx = tx.clone();
    peer_map.lock().unwrap().insert(addr, tx);

    let (outgoing, incoming) = ws_stream.split();

    let receive = rx.map(Ok).forward(outgoing);
    tokio::spawn(async move {
        loop {
            ping_tx.unbounded_send(Message::Ping(vec![0u8])).unwrap();
            tokio::time::sleep(tokio::time::Duration::from_millis(20000)).await;
        }
    });

    pin_mut!(receive);
    receive.await;

    peer_map.lock().unwrap().remove(&addr);

}
