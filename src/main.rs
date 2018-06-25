#![deny(warnings)]

extern crate bincode;
extern crate bytes;
extern crate state;
extern crate time;
extern crate tokio;
extern crate tokio_codec;

extern crate p2p_poc;

#[macro_use]
extern crate lazy_static;

use bincode::{serialize, deserialize};

use bytes::BufMut;

use state::Storage;

use std::collections::HashMap;
use std::io;
use std::sync::Mutex;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use tokio::net::{TcpStream, TcpListener};
use tokio::prelude::*;
use tokio::runtime::Runtime;
use tokio_codec::Decoder;

use p2p_poc::p2p::*;

lazy_static! {
    static ref GLOBAL_INBOUND_NODES_MAP: Storage<Mutex<HashMap<u64, Node>>> = Storage::new();
    static ref GLOBAL_ACTIVE_NODES_MAP: Storage<Mutex<HashMap<u64, Node>>> = Storage::new();
    static ref GLOBAL_OUTBOUND_NODES_MAP: Storage<Mutex<HashMap<u64, Node>>> = Storage::new();
}

fn main() {
    let node_id = "c33d4c07-6a29-4ca6-8b06-b2781ba7f9b0";
    let net_id = 666;
    let local_addr = "127.0.0.1:8080";
    let mut peer_addrs = Vec::new();
    peer_addrs.push("127.0.0.1:8081");
    peer_addrs.push("127.0.0.1:8082");
    
    let mut rt = Runtime::new().unwrap();
    
    let inbound_nodes = HashMap::new();
    let active_nodes = HashMap::new();
    let outbound_nodes = HashMap::new();
    GLOBAL_INBOUND_NODES_MAP.set(Mutex::new(inbound_nodes));
    GLOBAL_ACTIVE_NODES_MAP.set(Mutex::new(active_nodes));
    GLOBAL_OUTBOUND_NODES_MAP.set(Mutex::new(outbound_nodes));
    
    let listener = TcpListener::bind(&local_addr.to_string().parse().unwrap()).expect("failed to bind");
    println!("Listening on: {}", local_addr);
    
    let server = listener.incoming()
        .map_err(|e| println!("failed to accept socket; error = {:?}", e))
        .for_each(move|socket| {
            let peer_addr = socket.peer_addr().unwrap();
            println!("New Connection: {}", peer_addr);
            
            let mut node = Node::new();
            let ip = socket.peer_addr().unwrap().ip().to_string();
            let ip: Vec<&str> = ip.split(".").collect();
            let mut pos = 0;
            for sec in ip.iter() {
                node.ip[pos] = sec.parse::<u8>().unwrap();
                pos = pos + 1;
            }
            let port = socket.peer_addr().unwrap().port();
            node.port = port as u32;
            
            
            // add incoming peer into inbound nodes list
            let mut inbound_nodes = GLOBAL_INBOUND_NODES_MAP.get().lock().unwrap();
            inbound_nodes.insert(calculate_hash(&node), node);
            println!("inbound nodes list size: {}.", inbound_nodes.len());
            
            println!("{}", node);
            println!("hash: code {:064X}", calculate_hash(&node));
            
            process(socket);
            Ok(())
        });
    rt.spawn(server);
    
    for peer_addr in peer_addrs.iter() {
        let connect = TcpStream::connect(&peer_addr.to_string().parse().unwrap())
            .map(move|socket| {
                println!("Connected");
                
                let mut node = Node::new();
                
                let local_ip = socket.local_addr().unwrap().ip().to_string();
                let local_ip: Vec<&str> = local_ip.split(".").collect();
                let local_port = socket.local_addr().unwrap().port();
                
                let peer_ip = socket.peer_addr().unwrap().ip().to_string();
                let peer_ip: Vec<&str> = peer_ip.split(".").collect();
                let peer_port = socket.peer_addr().unwrap().port();
                
                let mut pos = 0;
                for sec in peer_ip.iter() {
                    node.ip[pos] = sec.parse::<u8>().unwrap();
                    pos = pos + 1;
                }
                node.port = peer_port as u32;
                
                // add connected peer into outbound nodes list
                let mut outbound_nodes = GLOBAL_OUTBOUND_NODES_MAP.get().lock().unwrap();
                let node_id_hash = calculate_hash(&node);
                node.id_hash = node_id_hash;
                
                println!("{}", node);
                println!("hash: code {:064X}", node_id_hash);
                
                outbound_nodes.insert(node_id_hash, node);
                
                println!("outbound nodes list size: {}.", outbound_nodes.len());
                
                let (mut tx, rx) =
                    P2p.framed(socket)
                        .split();
                
                let mut req = ChannelBuffer::new();
                req.head.ver = Version::V0;
                req.head.ctrl = Control::NET;
                req.head.action = Action::HANDSHAKEREQ;
                req.head.node_id_hash = node_id_hash;
                
                let mut body_req = HandshakeReqBody::new();
                let node_id_secs:Vec<&str> = node_id.split("-").collect();
                body_req.node_id_sec1.copy_from_slice(node_id_secs[0].to_string().into_bytes().as_slice());
                body_req.node_id_sec2.copy_from_slice(node_id_secs[1].to_string().into_bytes().as_slice());
                body_req.node_id_sec3.copy_from_slice(node_id_secs[2].to_string().into_bytes().as_slice());
                body_req.node_id_sec4.copy_from_slice(node_id_secs[3].to_string().into_bytes().as_slice());
                body_req.node_id_sec5.copy_from_slice(node_id_secs[4].to_string().into_bytes().as_slice());
                body_req.net_id = net_id;
                let mut pos = 0;
                for sec in local_ip.iter() {
                    body_req.ip[pos] = sec.parse::<u8>().unwrap();
                    pos = pos + 1;
                }
                body_req.port = local_port as u32;
                body_req.revision_version.push(4);
                body_req.revision_version.put_slice("Aion".to_string().into_bytes().as_slice());
                body_req.revision_version.push(2);
                body_req.revision_version.put_slice("01".to_string().into_bytes().as_slice());
                
                let encoded: Vec<u8> = serialize(&body_req).unwrap();
                req.body.put_slice(encoded.as_slice());
                req.head.len = req.body.len() as u32;
    
                println!("node_id_hash: {:064X}", req.head.node_id_hash);
                println!("body_req: {}", body_req);
                
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
            res.head.ctrl = Control::UNKNOWN;
            
            match req.head.ctrl {
                Control::NET => {
                    println!("P2P message received.");
                    
                    res.head.ctrl = Control::NET;
                    res.head.action = Action::UNKNOWN;
                    
                    match req.head.action {
                        Action::DISCONNECT => {
                            println!("DISCONNECT action received.");
                        }
                        Action::HANDSHAKEREQ => {
                            println!("HANDSHAKEREQ action received.");
                            
                            res.head.action = Action::HANDSHAKERES;
                            res.body.put_slice(handle_handshake_req(req.body).as_slice());
                            res.head.node_id_hash = req.head.node_id_hash;
                        }
                        Action::HANDSHAKERES => {
                            println!("HANDSHAKERES action received.");
                            handle_handshake_res(req.head.node_id_hash);
                        }
                        Action::PING => {
                            println!("PING action received.");
                            
                            res.head.action = Action::PONG;
                            res.body.put_slice("Aion pong".to_string().as_bytes().to_vec().as_slice());
                        }
                        Action::PONG => {
                            println!("PONG action received.");
                        }
                        Action::ACTIVENODESREQ => {
                            println!("ACTIVENODESREQ action received.");
                            
                            res.head.action = Action::ACTIVENODESRES;
                            res.body.put_slice("Aion ACTIVENODESRES id: 0001".to_string().as_bytes().to_vec().as_slice());
                        }
                        Action::ACTIVENODESRES => {
                            println!("ACTIVENODESRES action received.");
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
    
    let mut inbound_nodes = GLOBAL_INBOUND_NODES_MAP.get().lock().unwrap();
    println!("inbound nodes list size: {}.", inbound_nodes.len());
    inbound_nodes.remove(&calculate_hash(&node));
    println!("inbound nodes list size: {}.", inbound_nodes.len());
    
    node.id_sec1.copy_from_slice(decoded.node_id_sec1.to_vec().as_slice());
    node.id_sec2.copy_from_slice(decoded.node_id_sec2.to_vec().as_slice());
    node.id_sec3.copy_from_slice(decoded.node_id_sec3.to_vec().as_slice());
    node.id_sec4.copy_from_slice(decoded.node_id_sec4.to_vec().as_slice());
    node.id_sec5.copy_from_slice(decoded.node_id_sec5.to_vec().as_slice());
    node.id_hash = calculate_hash(&node);
    println!("{}", node);
    println!("hash: code {:064X}", node.id_hash);
    
    let mut active_nodes = GLOBAL_ACTIVE_NODES_MAP.get().lock().unwrap();
    println!("active nodes list size: {}.", active_nodes.len());
    active_nodes.insert(node.id_hash, node);
    println!("active nodes list size: {}.", active_nodes.len());
    
    res_body
}

fn handle_handshake_res(node_id_hash: u64) {
    println!("node_id_hash is {:064X}.", node_id_hash);
    
    let outbound = GLOBAL_OUTBOUND_NODES_MAP.get().lock().unwrap();
    let node = outbound.get(&node_id_hash).unwrap();;
    
    println!("node: {}.", node);
    
    let mut outbound = GLOBAL_OUTBOUND_NODES_MAP.get().lock().unwrap();
    println!("outbound nodes list size: {}.", outbound.len());

    outbound.remove(&node_id_hash);
    println!("outbound nodes list size: {}.", outbound.len());
    
    let mut active_nodes = GLOBAL_ACTIVE_NODES_MAP.get().lock().unwrap();
    println!("active nodes list size: {}.", active_nodes.len());
    active_nodes.insert(node_id_hash, *node);
    println!("active nodes list size: {}.", active_nodes.len());
}

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}