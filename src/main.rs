use babilado_types::Event;
use jsonl::Connection;
use std::io;
use std::net::{TcpListener, TcpStream};

fn main() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:9999")?;
    let pool = threadpool::Builder::new().build();

    for stream in listener.incoming() {
        pool.execute(|| {
            if let Err(e) = handle_connection(stream) {
                eprintln!("Error: {:?}", e);
            }
        });
    }

    pool.join();

    Ok(())
}

fn handle_connection(stream: io::Result<TcpStream>) -> anyhow::Result<()> {
    let stream = stream?;
    let mut connection = Connection::new_from_tcp_stream(stream)?;

    loop {
        let event: Event = connection.read()?;
        dbg!(&event);
        connection.write(&event)?;
    }
}
