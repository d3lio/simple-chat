use std::net::TcpStream;
use std::io::Write;

fn main() {
    let mut stream = TcpStream::connect("127.0.0.1:12345").unwrap();

    for i in 0..10 {
        let size = 10u64;
        stream.write_all(unsafe { &::std::mem::transmute::<u64, [u8; 8]>(u64::to_le(size)) }).unwrap();
        stream.write_fmt(format_args!("0123456789")).unwrap();
        println!("sent message no. {}", i);
    }
}
