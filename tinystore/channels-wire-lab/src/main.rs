use channels_wire_lab::Hub;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    time::{Duration, MissedTickBehavior},
};

async fn serve<S: AsyncReadExt + AsyncWriteExt + Unpin>(mut socket: S, hub: std::sync::Arc<Hub>) {
    let mut connection = hub.connect();
    let mut buffer = vec![0; 65536];
    let mut tick = tokio::time::interval(Duration::from_millis(1));
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        tokio::select! {r=socket.read(&mut buffer)=>match r{Ok(0)|Err(_)=>break,Ok(n)=>connection.push(&buffer[..n])},_=tick.tick()=>{}}
        let bytes = connection.take(256 * 1024);
        if !bytes.is_empty() && socket.write_all(&bytes).await.is_err() {
            break;
        }
    }
}
#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() {
    let a: Vec<String> = std::env::args().collect();
    let hub = Hub::open(&a[1], a[2] == "ring");
    if a[3] == "tcp" {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:16979")
            .await
            .unwrap();
        println!("ready");
        loop {
            let (socket, _) = listener.accept().await.unwrap();
            socket.set_nodelay(true).unwrap();
            tokio::spawn(serve(socket, hub.clone()));
        }
    } else {
        let listener = tokio::net::UnixListener::bind(&a[4]).unwrap();
        println!("ready");
        loop {
            let (socket, _) = listener.accept().await.unwrap();
            tokio::spawn(serve(socket, hub.clone()));
        }
    }
}
