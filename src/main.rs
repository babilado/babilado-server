use babilado_types::Event;
use std::net::SocketAddr;
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

#[derive(Debug)]
struct Client {
    writer: OwnedWriteHalf,
    address: SocketAddr,
}

async fn forward_events(
    mut clients_rx: mpsc::Receiver<Client>,
    mut events_rx: mpsc::Receiver<(Event, SocketAddr)>,
) -> anyhow::Result<()> {
    let mut clients = Vec::new();

    loop {
        select! {
            Some(client) = clients_rx.recv() => clients.push(client),
            Some((event, event_origin)) = events_rx.recv() => {
                for Client { writer, address } in &mut clients {
                    if *address == event_origin {
                        continue;
                    }

                    jsonl::write(writer, &event).await?;
                }
            },
            else => return Ok(()),
        }
    }
}

async fn handle_stream(
    stream: io::Result<TcpStream>,
    clients_tx: mpsc::Sender<Client>,
    events_tx: mpsc::Sender<(Event, SocketAddr)>,
) -> anyhow::Result<()> {
    let stream = stream?;

    let address = stream.peer_addr()?;
    let (read_half, write_half) = stream.into_split();

    clients_tx
        .send(Client {
            writer: write_half,
            address,
        })
        .await
        .unwrap();

    let mut read_half = BufReader::new(read_half);
    loop {
        let event: Event = jsonl::read(&mut read_half).await?;
        events_tx.send((event, address)).await.unwrap();
    }
}
