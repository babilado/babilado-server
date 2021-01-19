use babilado_types::Event;
use jsonl::Connection;
use tokio::net::{TcpListener, TcpStream};
use tokio::{io, task};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:9999").await?;

    loop {
        let stream = listener.accept().await.map(|(stream, _)| stream);

        task::spawn(async {
            if let Err(e) = handle_connection(stream).await {
                eprintln!("Error: {:?}", e);
            }
        });
    }
}

async fn handle_connection(stream: io::Result<TcpStream>) -> anyhow::Result<()> {
    let mut stream = stream?;
    let mut connection = Connection::new_from_tcp_stream(&mut stream)?;

    loop {
        let event: Event = connection.read().await?;
        dbg!(&event);
        connection.write(&event).await?;
    }
}
