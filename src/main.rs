#![deny(warnings)]

extern crate bytes;

extern crate time;
extern crate tokio;
extern crate tokio_codec;
extern crate bincode;

extern crate p2p_poc;

#[macro_use]
extern crate lazy_static;

use bincode::{serialize, deserialize};

use bytes::BufMut;

use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use tokio::net::{TcpStream, TcpListener};
use tokio::prelude::*;
use tokio::runtime::Runtime;
use tokio_codec::Decoder;

use p2p_poc::p2p::*;

lazy_static! {
    static ref INBOUND_NODES: Arc<Mutex<HashMap<u64, Node>>> = Arc::new(Mutex::new(HashMap::new()));
}

fn main() {
    let mut rt = Runtime::new().unwrap();

    let local_addr = "127.0.0.1:8081".parse().unwrap();
    let listener = TcpListener::bind(&local_addr).expect("failed to bind");
    println!("Listening on: {}", local_addr);

    let server = listener.incoming()
        .map_err(|e| println!("failed to accept socket; error = {:?}", e))
        .for_each(move|socket| {
            let peer_addr = socket.peer_addr().unwrap();
            println!("New Connection: {}", peer_addr);

            process(socket);
            Ok(())
        });
    rt.spawn(server);

    let mut peer_addrs = Vec::new();
    peer_addrs.push("127.0.0.1:8080".parse().unwrap());
    peer_addrs.push("127.0.0.1:8082".parse().unwrap());

    for peer_addr in peer_addrs.iter() {
        let nodes = INBOUND_NODES.clone();
        let connect = TcpStream::connect(&peer_addr)
            .map(move|socket| {
                println!("Connected");

                let mut node = Node::new();
                let ip = socket.peer_addr().unwrap().ip();
                let port = socket.peer_addr().unwrap().port();

                println!("ip len: {}", ip.to_string().into_bytes().len());
                println!("ip len: {}", ip.to_string());

                node.ip.copy_from_slice(ip.to_string().into_bytes().as_slice());
                node.port = port as u32;
                nodes.lock().unwrap().insert(calculate_hash(&node), node);

                println!("nodes size: {}.", nodes.lock().unwrap().len());

                let (mut tx, rx) =
                    P2p.framed(socket)
                        .split();

                let mut req = ChannelBuffer::new();
                req.head.ver = Version::V0;
                req.head.ctrl = Control::NET;
                req.head.action = Action::HANDSHAKEREQ;

                let mut body_req = HandshakeReqBody::new();
                body_req.node_id_sec1.copy_from_slice("000000000000000001".to_string().into_bytes().as_slice());
                body_req.node_id_sec2.copy_from_slice("000000000000000001".to_string().into_bytes().as_slice());
                body_req.net_id = 666;
                body_req.ip.copy_from_slice("00000002".to_string().into_bytes().as_slice());
                body_req.port = 8888;
                body_req.revision_version.push(4);
                body_req.revision_version.put_slice("Aion".to_string().into_bytes().as_slice());
                body_req.revision_version.push(2);
                body_req.revision_version.put_slice("01".to_string().into_bytes().as_slice());

                let encoded: Vec<u8> = serialize(&body_req).unwrap();
                req.body.put_slice(encoded.as_slice());
                req.head.len = req.body.len() as u32;

                tx.start_send(req).unwrap();

                let task = tx.send_all(rx.and_then(respond).filter(|item| item.head.action != Action::UNKNOWN))
                    .then(|res| {
                        if let Err(e) = res {
                            println!("failed to process connection; error = {:?}", e);
                        }

                        Ok(())
                    });

                tokio::spawn(task);
            })
            .map_err(|e| println!("Failed to connect: {}", e));
        rt.spawn(connect);
    }

    rt.shutdown_on_idle()
        .wait().unwrap();
}

fn process(socket: TcpStream) {
    let (tx, rx) =
        P2p.framed(socket)
            .split();

    let task = tx.send_all(rx.and_then(respond).filter(|item| item.head.action != Action::UNKNOWN))
        .then(|res| {
            if let Err(e) = res {
                println!("failed to process connection; error = {:?}", e);
            }

            Ok(())
        });

    tokio::spawn(task);
}


fn respond(req: ChannelBuffer)
           -> Box<Future<Item=ChannelBuffer, Error=io::Error> + Send>
{
    let mut res = ChannelBuffer::new();

    match req.head.ver {
        Version::V0 => {
            println!("Ver 0 package received.");

            res.head.ver = Version::V0;

            match req.head.ctrl {
                Control::NET => {
                    println!("P2P message received.");

                    res.head.ctrl = Control::NET;

                    match req.head.action {
                        Action::DISCONNECT => {
                            println!("DISCONNECT action received.");

                            res.head.action = Action::UNKNOWN;
                        }
                        Action::HANDSHAKEREQ => {
                            println!("HANDSHAKEREQ action received.");


                            res.head.action = Action::HANDSHAKERES;
                            res.body.put_slice(handle_handshake_req(req.body).as_slice());
                        }
                        Action::HANDSHAKERES => {
                            println!("HANDSHAKERES action received.");

                            res.head.action = Action::PING;
                            res.body.put_slice("Aion ping".to_string().as_bytes().to_vec().as_slice());
                        }
                        Action::PING => {
                            println!("PING action received.");

                            res.head.action = Action::PONG;
                            res.body.put_slice("Aion pong".to_string().as_bytes().to_vec().as_slice());
                        }
                        Action::PONG => {
                            println!("PONG action received.");

                            res.head.action = Action::ACTIVENODESREQ;
                            res.body.put_slice("Aion ACTIVENODESREQ id: 0001".to_string().as_bytes().to_vec().as_slice());
                        }
                        Action::ACTIVENODESREQ => {
                            println!("ACTIVENODESREQ action received.");

                            res.head.action = Action::ACTIVENODESRES;
                            res.body.put_slice("Aion ACTIVENODESRES id: 0001".to_string().as_bytes().to_vec().as_slice());
                        }
                        Action::ACTIVENODESRES => {
                            println!("ACTIVENODESRES action received.");

                            res.head.action = Action::DISCONNECT;
                            res.body.put_slice("Aion DISCONNECT.".to_string().as_bytes().to_vec().as_slice());
                        }
                        _ => {
                            println!("Invalid action received.");
                        }
                    }
                }
                Control::SYNC => {
                    println!("Kernel message received.");
                }
                _ => {
                    println!("Invalid message received.");
                }
            }
        }
        Version::V1 => {
            println!("Ver 1 package received.");
        }
        _ => {
            println!("Invalid Version.");
        }
    };

    res.head.len = res.body.len() as u32;

    return Box::new(future::ok(res));
}


fn handle_handshake_req(req_body: Vec<u8>)
                        -> Vec<u8>
{
    let mut res_body = Vec::new();

    let encoded: Vec<u8> = req_body.to_vec();
    let decoded: HandshakeReqBody = deserialize(&encoded[..]).unwrap();
    println!("{}", decoded);

    let revision_version = decoded.revision_version;
    let (revision_len, rest) = revision_version.split_at(1);
    let revision_len = revision_len[0] as usize;
    let (revision, rest) = rest.split_at(revision_len);
    let (version_len, rest) = rest.split_at(1);
    let version_len = version_len[0] as usize;
    let (_version, _rest) = rest.split_at(version_len);


    let mut body = HandshakeResBody::new();
    body.status = 0;
    body.binary_version.put_slice(revision.to_vec().as_slice());
    body.len = body.binary_version.len() as u8;

    let encoded: Vec<u8> = serialize(&body).unwrap();
    res_body.put_slice(encoded.as_slice());

    // move inbound node to active
    let mut node = Node::new();
    node.ip.copy_from_slice(decoded.ip.to_vec().as_slice());
    node.port = decoded.port;
    let nodes = INBOUND_NODES.clone();
    nodes.lock().unwrap().remove(&calculate_hash(&node));
    println!("nodes size: {}.", nodes.lock().unwrap().len());

    res_body
}

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}
