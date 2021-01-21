use babilado_types::{Event, User};
use jsonl::ReadError;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{self, BufReader};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tokio::{select, task};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let users = Arc::new(Mutex::new(Vec::new()));
    let (clients_tx, clients_rx) = mpsc::channel(100);
    let (events_tx, events_rx) = mpsc::channel(100);
    task::spawn(forward_events(clients_rx, events_rx));

    let listener = TcpListener::bind("127.0.0.1:9999").await?;

    loop {
        let stream = listener.accept().await.map(|(stream, _)| stream);

        let clients_tx = clients_tx.clone();
        let events_tx = events_tx.clone();

        let users = Arc::clone(&users);
        task::spawn(async {
            if let Err(e) = handle_stream(stream, users, clients_tx, events_tx).await {
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

#[derive(Debug)]
enum ClientEvent {
    Add(Client),
    Remove(SocketAddr),
}

async fn forward_events(
    mut clients_rx: mpsc::Receiver<ClientEvent>,
    mut events_rx: mpsc::Receiver<(Event, SocketAddr)>,
) -> anyhow::Result<()> {
    let mut clients = Vec::new();

    loop {
        select! {
            Some(client_event) = clients_rx.recv() => {
                handle_client_event(client_event, &mut clients)
            },
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

fn handle_client_event(client_event: ClientEvent, clients: &mut Vec<Client>) {
    match client_event {
        ClientEvent::Add(client) => clients.push(client),
        ClientEvent::Remove(address) => {
            let idx = clients
                .iter()
                .enumerate()
                .find_map(
                    |(idx, Client { address: a, .. })| if *a == address { Some(idx) } else { None },
                )
                .unwrap();

            clients.remove(idx);
        }
    }
}

async fn handle_stream(
    stream: io::Result<TcpStream>,
    users: Arc<Mutex<Vec<User>>>,
    clients_tx: mpsc::Sender<ClientEvent>,
    events_tx: mpsc::Sender<(Event, SocketAddr)>,
) -> anyhow::Result<()> {
    let mut stream = stream?;
    let user: User = jsonl::read(BufReader::new(&mut stream)).await?;
    if users
        .lock()
        .await
        .iter()
        .any(|u| u.nickname == user.nickname && u.tag == user.tag)
    {
        jsonl::write(
            &mut stream,
            &"Nickname and tag combination taken. Try a something else!",
        )
        .await?;
        anyhow::bail!("Nickname and tag comination already in use");
    }
    users.lock().await.push(user);
    let address = stream.peer_addr()?;
    let (read_half, write_half) = stream.into_split();

    clients_tx
        .send(ClientEvent::Add(Client {
            writer: write_half,
            address,
        }))
        .await
        .unwrap();

    let mut read_half = BufReader::new(read_half);

    loop {
        let event: Event = match jsonl::read(&mut read_half).await {
            Ok(e) => e,
            Err(ReadError::Eof) => {
                clients_tx.send(ClientEvent::Remove(address)).await.unwrap();
                return Ok(());
            }
            Err(e) => return Err(e.into()),
        };

        events_tx.send((event, address)).await.unwrap();
    }
}
