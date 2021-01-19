use babilado_types::Event;
use tokio::io::{self, BufReader};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::{select, task};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (clients_tx, clients_rx) = mpsc::channel(100);
    let (events_tx, events_rx) = mpsc::channel(100);
    task::spawn(forward_events(clients_rx, events_rx));

    let listener = TcpListener::bind("127.0.0.1:9999").await?;

    loop {
        let stream = listener.accept().await.map(|(stream, _)| stream);

        let clients_tx = clients_tx.clone();
        let events_tx = events_tx.clone();

        task::spawn(async {
            if let Err(e) = handle_stream(stream, clients_tx, events_tx).await {
                eprintln!("Error: {:?}", e);
            }
        });
    }
}

async fn forward_events(
    mut clients_rx: mpsc::Receiver<OwnedWriteHalf>,
    mut events_rx: mpsc::Receiver<Event>,
) -> anyhow::Result<()> {
    let mut clients = Vec::new();

    loop {
        select! {
            Some(client) = clients_rx.recv() => clients.push(client),
            Some(event) = events_rx.recv() => {
                for client in &mut clients {
                    jsonl::write(client, &event).await?;
                }
            },
            else => return Ok(()),
        }
    }
}

async fn handle_stream(
    stream: io::Result<TcpStream>,
    clients_tx: mpsc::Sender<OwnedWriteHalf>,
    events_tx: mpsc::Sender<Event>,
) -> anyhow::Result<()> {
    let stream = stream?;
    let (read_half, write_half) = stream.into_split();

    clients_tx.send(write_half).await.unwrap();

    let mut read_half = BufReader::new(read_half);
    loop {
        let event: Event = jsonl::read(&mut read_half).await?;
        events_tx.send(event).await.unwrap();
    }
}
