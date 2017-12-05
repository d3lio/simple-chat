use std::io::{Read, Write, ErrorKind};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{channel, Sender, Receiver, TryRecvError};
use std::thread;

const URL: &str = "127.0.0.1:12345";
const BUFFER_SIZE: usize = 128;

#[derive(Debug, PartialEq, Eq)]
enum MessagePhase {
    Header,
    Payload
}

#[derive(Debug)]
struct Message {
    size: u64,
    buffer: Vec<u8>,
    phase: MessagePhase
}

impl Message {
    pub fn new() -> Self {
        Self {
            size: 0,
            buffer: Vec::with_capacity(BUFFER_SIZE),
            phase: MessagePhase::Header
        }
    }

    // TODO bufreader
    fn read_header(&mut self, stream: &mut TcpStream) -> bool {
        if self.phase != MessagePhase::Header {
            return false;
        }

        self.size = unsafe {
            let mut buffer = [0u8; 8];
            if let Err(err) = stream.read_exact(&mut buffer) {
                match err.kind() {
                    ErrorKind::WouldBlock => return false,
                    ErrorKind::UnexpectedEof => {
                        self.phase = MessagePhase::Payload;
                        return true;
                    },
                    _ => panic!("{:?}", err)
                }
            }

            let num = ::std::mem::transmute(buffer);
            if num == 0 {
                return true;
            }
            u64::from_le(num)
        };

        self.phase = MessagePhase::Payload;
        true
    }

    // TODO bufreader
    fn read_payload(&mut self, stream: &mut TcpStream) -> bool {
        if self.phase != MessagePhase::Payload {
            return false;
        }

        if self.size > self.buffer.len() as u64 {
            if self.buffer.capacity() - self.buffer.len() < BUFFER_SIZE {
                let cap = self.buffer.capacity();
                self.buffer.resize(cap + BUFFER_SIZE, 0);
            }

            let orig_len = self.buffer.len();
            let cap = self.buffer.capacity();
            unsafe {
                self.buffer.set_len(cap);
                match stream.read(&mut self.buffer[orig_len..(orig_len + BUFFER_SIZE)]) {
                    Ok(len) => {
                        self.buffer.set_len(orig_len + len);
                        if self.size <= self.buffer.len() as u64 {
                            true
                        } else {
                            false
                        }
                    },
                    Err(err) => {
                        self.buffer.set_len(orig_len);
                        if err.kind() == ErrorKind::WouldBlock {
                            false
                        } else {
                            panic!(err);
                        }
                    }
                }
            }
        } else {
            true
        }
    }

    pub fn read(&mut self, stream: &mut TcpStream) -> bool {
        match self.phase {
            MessagePhase::Header => self.read_header(stream) && self.read_payload(stream),
            MessagePhase::Payload => self.read_payload(stream)
        }
    }
}

/*fn read_proto(stream: &mut TcpStream) -> (u64, Vec<u8>) {
    let message_size: u64 = unsafe {
        let mut buffer = [0u8; 8];
        stream.read_exact(&mut buffer).unwrap();

        let num = ::std::mem::transmute(buffer);
        if num == 0 {
            return (0, Vec::new());
        }
        u64::from_le(num)
    };

    let mut buffer = Vec::with_capacity(BUFFER_SIZE);
    while (buffer.len() as u64) < message_size {
        if buffer.capacity() - buffer.len() < BUFFER_SIZE {
            let cap = buffer.capacity();
            buffer.resize(cap + BUFFER_SIZE, 0);
        }

        let orig_len = buffer.len();
        let cap = buffer.capacity();
        unsafe {
            buffer.set_len(cap);
            let len = stream.read(&mut buffer[orig_len..(orig_len + BUFFER_SIZE)]).unwrap();
            buffer.set_len(orig_len + len);
        }
    }
    (message_size, buffer)
}*/

struct BiChannel<T> {
    pub sender: Sender<T>,
    pub receiver: Receiver<T>
}

impl<T> BiChannel<T> {
    pub fn new() -> (Self, Self) {
        let (sender1, receiver1) = channel();
        let (sender2, receiver2) = channel();
        (Self {
            sender: sender1,
            receiver: receiver2
        },
        Self {
            sender: sender2,
            receiver: receiver1
        })
    }
}

fn main() {
    let server = TcpListener::bind(URL).unwrap();
    server.set_nonblocking(true).unwrap();
    let mut clients = Vec::new();

    loop {
        // Try to accept a connection
        if let Ok((mut stream, addr)) = server.accept() {
            println!("Connection from addr {}", addr);

            let channel = {
                let (a, b) = BiChannel::new();
                clients.push(a);
                b
            };

            thread::spawn(move || {
                stream.set_nonblocking(true).expect("set_nonblocking failed");

                let mut message = Message::new();

                // TODO: park
                // event loop
                loop {
                    if message.read(&mut stream) {
                        if message.size == 0 {
                            break;
                        }

                        println!("{:?}", message);

                        if message.size < message.buffer.len() as u64 {
                            stream.write_fmt(format_args!{
                                "Message size missmatch. Expected {} bytes, read {} bytes",
                                message.size,
                                message.buffer.len()
                            }).expect("write_fmt failed");
                        } else {
                            channel.sender.send(message.buffer).expect("send failed");
                        }

                        message = Message::new();
                    }

                    if let Ok(data) = channel.receiver.try_recv() {
                        stream.write_all(&data[..]).expect("write_all failed");
                    }

                    ::std::thread::sleep(::std::time::Duration::from_millis(100));
                }

                println!("Connection closed addr {}", addr);
            });
        }

        // let mut data = Vec::<u8>::new();
        clients.retain(|client| {
            match client.receiver.try_recv() {
                Ok(data) => {
                    println!("{:?}", data);
                    // data = data;
                    true
                },
                Err(TryRecvError::Empty) => true,
                Err(TryRecvError::Disconnected) => false
            }
        });
        // if data.len() {
        //     for client in clients.iter() {
        //         client.sender.send
        //     }
        // }

        ::std::thread::sleep(::std::time::Duration::from_millis(100));
    }
}
