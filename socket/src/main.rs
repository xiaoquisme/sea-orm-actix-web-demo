pub mod my_server {
    use std::{io::{self, Write}, net::{TcpListener, TcpStream}, thread::spawn};

    pub fn echo_main(addr: &str) -> io::Result<()> {
        let listener = TcpListener::bind(addr)?;
        println!("tcp listen on {}", addr);
        loop {
            let (mut _stream, addr) = listener.accept()?;
            println!("connected reveived from : {}", addr);
            let mut write_steam = _stream.try_clone()?;
            spawn( move || {
                io::copy(&mut _stream, &mut write_steam).expect("error in client mode");
                println!("connectino closed");
            });
        }
    }
}

use my_server::echo_main;

fn main() {
    echo_main("127.0.0.1:12345").expect("error: ");
}
