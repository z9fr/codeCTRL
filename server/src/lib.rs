use std::{
    error::Error,
    sync::{
        mpsc::{sync_channel, Receiver, SyncSender},
        Arc,
    },
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpSocket,
    runtime::Runtime,
};

#[derive(Clone)]
pub struct Server {
    sender: Arc<SyncSender<String>>,
    port: String,
}

impl Server {
    pub fn new(port: &str) -> (Self, Receiver<String>) {
        let (sender, receiver) = sync_channel(2);

        (
            Self {
                sender: Arc::new(sender),
                port: port.into(),
            },
            receiver,
        )
    }

    pub fn run_server(&mut self) -> Result<(), Box<dyn Error>> {
        let rt = Runtime::new()?;

        let mut ret = Ok(());

        rt.block_on(async {
            ret = self._run_server().await;
        });

        ret
    }

    async fn _run_server(&mut self) -> Result<(), Box<dyn Error>> {
        let socket = TcpSocket::new_v4()?;
        socket.set_reuseaddr(true)?;
        #[cfg(unix)]
        socket.set_reuseport(true)?; // If we're using a *NIX system, allow for multiple
                                     // instances of codeCTRL to use the same port.
                                     // *However*, this will cause some instances to
                                     // receive POST data but some others will not.

        socket.bind(format!("127.0.0.1:{}", self.port).parse().unwrap())?;

        let listener = socket.listen(1024)?;

        loop {
            let (mut socket, _) = listener.accept().await?;

            let mut buf = [0; 1024];

            loop {
                let n = match socket.read(&mut buf).await {
                    Ok(n) if n == 0 => break,
                    Ok(n) => n,
                    Err(e) => {
                        eprintln!("Failed to read from socket: {}", e);
                        break;
                    },
                };

                if let Err(e) = socket.write_all(&buf[0..n]).await {
                    eprintln!("Failed to write to socket: {:?}", e);
                    break;
                }

                let mut string = String::from_utf8(buf[..n].to_vec())
                    .expect("Could not convert socket byffer to valid UTF-8 string");

                let index = string.find("\r\n\r\n").unwrap_or_default();
                string = String::from_utf8(string.as_bytes()[index..].to_vec()).unwrap();
                string = string.replace("\r\n\r\n", "");

                if let Err(e) = self.sender.send(string) {
                    eprintln!("Failed to send through channel: {}", e);
                    break;
                }
            }
        }
    }
}
