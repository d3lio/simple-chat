use std::io::{ErrorKind, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;

fn sleep() {
    thread::sleep(::std::time::Duration::from_millis(100));
}

const LOCALHOST: &str = "127.0.0.1:1234";
const MESSAGE_SIZE: usize = 32;

fn main() {
    let server = TcpListener::bind(LOCALHOST).expect("Listener failed to bind");
    server.set_nonblocking(true).expect("Failed to initialize nonblocking");

    let mut clients = Vec::new();
    let (sx, rx) = mpsc::channel::<String>();

    loop {
        // Try to accept a client
        if let Ok((mut socket, addr)) = server.accept() {
            println!("Client {} connected", addr);

            let sx = sx.clone();

            clients.push(socket.try_clone().expect("Failed to clone client"));

            thread::spawn(move || loop {
                let mut buf = vec![0; MESSAGE_SIZE];

                // Try to receive message from client
                match socket.read_exact(&mut buf) {
                    Ok(_) => {
                        let msg = buf.into_iter().take_while(|&x| x != 0).collect::<Vec<_>>();
                        let msg = String::from_utf8(msg).expect("Invalid utf8 message");

                        println!("{}: {:?}", addr, msg);
                        sx.send(msg).expect("Send to master channel failed");
                    },
                    Err(ref err) if err.kind() == ErrorKind::WouldBlock => (),
                    Err(_) => {
                        println!("Closing connection with: {}", addr);
                        break;
                    }
                }

                sleep();
            });
        }

        let recv_result = rx.try_recv().map(|msg| {
            let mut msg = msg.into_bytes();
            msg.resize(MESSAGE_SIZE, 0);
            msg
        });

        if let Ok(msg) = recv_result {
            // Try to send message from master channel
            clients = clients.into_iter().filter_map(|mut client| {
                if client.write_all(&msg).is_ok() {
                    Some(client)
                } else {
                    None
                }
            }).collect::<Vec<_>>();
        }

        sleep();
    }
}
