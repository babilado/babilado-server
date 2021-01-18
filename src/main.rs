use babilado_types::Event;
use jsonl::Connection;
use std::net::TcpListener;

fn main() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:9999")?;

    for stream in listener.incoming() {
        let stream = stream?;
        let mut connection = Connection::new_from_tcp_stream(stream)?;

        loop {
            let event: Event = connection.read()?;
            dbg!(&event);
            connection.write(&event)?;
        }
    }

    Ok(())
}
