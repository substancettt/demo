#![deny(warnings)]

extern crate bincode;
extern crate byteorder;
extern crate bytes;
extern crate dotenv;
extern crate state;
extern crate time;
extern crate tokio;
extern crate tokio_codec;

extern crate p2p_poc;

#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate log4rs;

use bincode::{config};

use byteorder::{BigEndian, ReadBytesExt};
use bytes::BufMut;

use dotenv::*;

use state::Storage;

use std::collections::HashMap;
use std::{env, io, mem};
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
    static ref GLOBAL_TEMP_NODES_MAP: Storage<Mutex<HashMap<u64, Node>>> = Storage::new();
}

fn main() {
    log4rs::init_file("config/log4rs.yaml", Default::default()).unwrap();
    info!("Booting up");

    let mut args = env::args();

    let mut config_file = "config/env".to_string();
    if args.len() == 2 {
        config_file= args.nth(1).unwrap();
    }

    info!("Configuration file {} loaded.", config_file);

    from_filename(config_file).ok();

    let mut rt = Runtime::new().unwrap();

    let inbound_nodes = HashMap::new();
    let active_nodes = HashMap::new();
    let outbound_nodes = HashMap::new();
    let temp_nodes = HashMap::new();
    GLOBAL_INBOUND_NODES_MAP.set(Mutex::new(inbound_nodes));
    GLOBAL_ACTIVE_NODES_MAP.set(Mutex::new(active_nodes));
    GLOBAL_OUTBOUND_NODES_MAP.set(Mutex::new(outbound_nodes));
    GLOBAL_TEMP_NODES_MAP.set(Mutex::new(temp_nodes));

    let local_addr = var("local_addr").unwrap().to_string();
    let listener = TcpListener::bind(&local_addr.parse().unwrap()).expect("failed to bind");
    info!("Listening on: {}", local_addr);

    let server = listener.incoming()
        .map_err(|e| error!("Failed to accept socket; error = {:?}", e))
        .for_each(move|socket| {
            process(socket);
            Ok(())
        });
    rt.spawn(server);

    let mut temp_nodes = GLOBAL_TEMP_NODES_MAP.get().lock().unwrap();
    let peer_nodes = var("peer_nodes").unwrap().to_string();
    let peer_nodes = peer_nodes.trim().split(",");
    for peer_node in peer_nodes {
        let (_, peer_node) = peer_node.split_at(6);
        let peer_node: Vec<&str> = peer_node.split("@").collect();
        let peer_node_id = peer_node[0];
        let peer_node_addr = peer_node[1];
        let peer_node: Vec<&str> = peer_node_addr.split(":").collect();
        let peer_node_ip = peer_node[0];
        let peer_node_port = peer_node[1];

        let mut node = Node::new();
        let peer_node_ip: Vec<&str> = peer_node_ip.split(".").collect();
        let mut pos = 0;
        for sec in peer_node_ip.iter() {
            node.ip_addr.ip[pos] = sec.parse::<u8>().unwrap();
            pos = pos + 1;
        }
        node.ip_addr.port = peer_node_port.parse::<u32>().unwrap();

        node.id_hash = calculate_hash(&node);
        let (id_sec1, id_sec2) = peer_node_id.split_at(24);
        node.id_sec1.copy_from_slice(id_sec1.as_bytes().to_vec().as_slice());
        node.id_sec2.copy_from_slice(id_sec2.as_bytes().to_vec().as_slice());

        temp_nodes.insert(node.id_hash, node);
    }

    for (_, temp_node) in temp_nodes.iter() {
        let peer_addr = temp_node.ip_addr.get_addr();
        let peer_node = temp_node.clone();
        let connect = TcpStream::connect(&peer_addr.parse().unwrap())
            .map(move|socket| {
                info!("Connected");

                let local_ip = socket.local_addr().unwrap().ip().to_string();
                let local_ip: Vec<&str> = local_ip.split(".").collect();
                let local_port = socket.local_addr().unwrap().port();

                let peer_node_hash = peer_node.id_hash;

                // add connected peer into outbound nodes list
                let mut outbound_nodes = GLOBAL_OUTBOUND_NODES_MAP.get().lock().unwrap();
                outbound_nodes.insert(peer_node_hash, peer_node);

                debug!("Add into outbound node list {}", peer_node);
                debug!("outbound nodes list size: {}.", outbound_nodes.len());

                let (mut tx, rx) =
                    P2p.framed(socket)
                        .split();

                let mut req = ChannelBuffer::new();
                req.head.ver = Version::V0.value();
                req.head.ctrl = Control::NET.value();
                req.head.action = Action::HANDSHAKEREQ.value();

                let mut encoder = config();
                let encoder = encoder.big_endian();

                let node_id = var("node_id").unwrap().to_string();
                req.body.put_slice(node_id.into_bytes().as_slice());

                let net_id = var("net_id").unwrap().parse::<u32>().unwrap();
                let net_id: Vec<u8> = encoder.serialize(&net_id).unwrap();
                req.body.put_slice(net_id.as_slice());

                let mut ip = [0; 8];
                let mut i = 0;
                for sec in local_ip.iter() {
                    ip[i] = sec.parse::<u8>().unwrap();
                    i = i + 1;
                }
                req.body.put_slice(ip.to_vec().as_slice());
                let port = local_port as u32;
                let port: Vec<u8> = encoder.serialize(&port).unwrap();
                req.body.put_slice(port.as_slice());
                req.body.push(4);
                req.body.put_slice("Aion".to_string().into_bytes().as_slice());
                req.body.push(1);
                req.body.put_slice("01".to_string().into_bytes().as_slice());

                req.head.len = req.body.len() as u32;

                tx.start_send(req).unwrap();

                let task = tx.send_all(rx.and_then(move| item| {
                    respond(item, peer_node_hash)
                }).filter(|item| Action::get(item.head.action) != Action::UNKNOWN))
                    .then(|res| {
                        if let Err(e) = res {
                            error!("Failed to process connection; error = {:?}", e);
                        }

                        Ok(())
                    });

                tokio::spawn(task);
            })
            .map_err(|e| error!("Failed to connect: {}", e));
        rt.spawn(connect);
    }

    rt.shutdown_on_idle()
        .wait().unwrap();
}

fn process(socket: TcpStream) {
    let peer_addr = socket.peer_addr().unwrap();
    info!("New Connection: {}", peer_addr);

    let mut node = Node::new();
    let ip = socket.peer_addr().unwrap().ip().to_string();
    let ip: Vec<&str> = ip.split(".").collect();
    let mut pos = 0;
    for sec in ip.iter() {
        node.ip_addr.ip[pos] = sec.parse::<u8>().unwrap();
        pos = pos + 1;
    }
    let port = socket.peer_addr().unwrap().port();
    node.ip_addr.port = port as u32;
    let node_id_hash = calculate_hash(&node);
    node.id_hash = node_id_hash;

    // add incoming peer into inbound nodes list
    let mut inbound_nodes = GLOBAL_INBOUND_NODES_MAP.get().lock().unwrap();
    inbound_nodes.insert(node_id_hash, node);

    debug!("{}", node);
    debug!("inbound nodes list size: {}.", inbound_nodes.len());

    let (tx, rx) =
        P2p.framed(socket)
            .split();

    let task = tx.send_all(rx.and_then(move| item| {
        respond(item, node_id_hash)
    }).filter(|item| Action::get(item.head.action) != Action::UNKNOWN))
        .then(|res| {
            if let Err(e) = res {
                error!("failed to process connection; error = {:?}", e);
            }

            Ok(())
        });

    tokio::spawn(task);
}

fn respond(req: ChannelBuffer, node_id_hash: u64)
           -> Box<Future<Item=ChannelBuffer, Error=io::Error> + Send>
{
    let mut res = ChannelBuffer::new();

    match Version::get(req.head.ver) {
        Version::V0 => {
            trace!("Ver 0 package received.");

            res.head.ver = Version::V0.value();
            res.head.ctrl = Control::UNKNOWN.value();

            match Control::get(req.head.ctrl) {
                Control::NET => {
                    trace!("P2P message received.");

                    res.head.ctrl = Control::NET.value();
                    res.head.action = Action::UNKNOWN.value();

                    match Action::get(req.head.action) {
                        Action::DISCONNECT => {
                            trace!("DISCONNECT action received.");
                        }
                        Action::HANDSHAKEREQ => {
                            trace!("HANDSHAKEREQ action received.");

                            res.head.action = Action::HANDSHAKERES.value();
                            res.body.put_slice(handle_handshake_req(req.body, node_id_hash).as_slice());
                        }
                        Action::HANDSHAKERES => {
                            debug!("HANDSHAKERES action received.");
                            handle_handshake_res(node_id_hash);

                            // for testing
                            res.head.action = Action::ACTIVENODESREQ.value();
                        }
                        Action::PING => {
                            trace!("PING action received.");

                            res.head.action = Action::PONG.value();
                            res.body.put_slice("Aion pong".to_string().as_bytes().to_vec().as_slice());
                        }
                        Action::PONG => {
                            trace!("PONG action received.");
                        }
                        Action::ACTIVENODESREQ => {
                            trace!("ACTIVENODESREQ action received.");

                            res.head.action = Action::ACTIVENODESRES.value();
                            res.body.put_slice(handle_active_nodes_req(node_id_hash).as_slice());
                        }
                        Action::ACTIVENODESRES => {
                            trace!("ACTIVENODESRES action received.");

                            handle_active_nodes_res(req.body);
                        }
                        _ => {
                            error!("Invalid action received.");
                        }
                    }
                }
                Control::SYNC => {
                    trace!("Kernel message received.");
                }
                _ => {
                    error!("Invalid message received.");
                }
            }
        }
        Version::V1 => {
            error!("Ver 1 package received.");
        }
        _ => {
            trace!("Invalid Version.");
        }
    };

    res.head.len = res.body.len() as u32;

    return Box::new(future::ok(res));
}


fn handle_handshake_req(body: Vec<u8>, node_id_hash: u64)
                        -> Vec<u8>
{
    let mut res_body = Vec::new();
    let mut req_body: HandshakeReqBody = HandshakeReqBody::new();

    let mut decoder = config();
    let decoder = decoder.big_endian();

    let (node_id, req_body_rest) = body.split_at(NODE_ID_LENGTH);
    req_body.node_id.copy_from_slice(node_id.to_vec().as_slice());
    let (net_id, req_body_rest) = req_body_rest.split_at(mem::size_of::<i32>());
    req_body.net_id = decoder.deserialize(net_id).unwrap();
    let (ip, req_body_rest) = req_body_rest.split_at(IP_LENGTH);
    req_body.ip_addr.ip.copy_from_slice(ip.to_vec().as_slice());
    let (port, revision_version) = req_body_rest.split_at(mem::size_of::<i32>());
    req_body.ip_addr.port = decoder.deserialize(port).unwrap();
    req_body.revision_version.put_slice(revision_version.to_vec().as_slice());

    debug!("HandshakeReqBody: {}", req_body);

    let revision_version = req_body.revision_version;
    let (revision_len, rest) = revision_version.split_at(1);
    let revision_len = revision_len[0] as usize;
    let (revision, rest) = rest.split_at(revision_len);
    let (version_len, rest) = rest.split_at(1);
    let version_len = version_len[0] as usize;
    let (_version, _rest) = rest.split_at(version_len);

    res_body.push(0 as u8);
    res_body.push(revision.len() as u8);
    res_body.put_slice(revision.to_vec().as_slice());

    // move inbound node to active
    let mut node = Node::new();
    node.ip_addr.ip.copy_from_slice(req_body.ip_addr.ip.to_vec().as_slice());
    node.ip_addr.port = req_body.ip_addr.port;
    node.id_hash = node_id_hash;
    let mut inbound_nodes = GLOBAL_INBOUND_NODES_MAP.get().lock().unwrap();

    match inbound_nodes.remove(&node_id_hash) {
        Some(node) => {
            debug!("inbound node list size: {}.", inbound_nodes.len());
            debug!("{}", node);

            let mut active_nodes = GLOBAL_ACTIVE_NODES_MAP.get().lock().unwrap();
            active_nodes.insert(node.id_hash, node);
            debug!("active nodes list size: {}.", active_nodes.len());
        }
        None => warn!("Node not found in inbound node list."),
    }
    res_body
}

fn handle_handshake_res(node_id_hash: u64) {
    let mut outbound = GLOBAL_OUTBOUND_NODES_MAP.get().lock().unwrap();

    // let node =
    match outbound.remove(&node_id_hash) {
        Some(node) => {
            debug!("node: {}", node);
            let mut active_nodes = GLOBAL_ACTIVE_NODES_MAP.get().lock().unwrap();
            active_nodes.insert(node_id_hash, node);
            debug!("active nodes list size: {}.", active_nodes.len());
        }
        None => warn!("Node not found in outbound node list."),
    }
}

fn handle_active_nodes_req(node_id_hash: u64)
                           -> Vec<u8>
{
    let mut res_body = Vec::new();

    let mut encoder = config();
    let encoder = encoder.big_endian();
    let active_nodes = GLOBAL_ACTIVE_NODES_MAP.get().lock().unwrap();
    match active_nodes.get(&node_id_hash) {
        Some(node) => {
            debug!("{}", node);

            res_body.push(active_nodes.len() as u8);
            for (_, node) in active_nodes.iter() {
                res_body.put_slice(node.id_sec1.to_vec().as_slice());
                res_body.put_slice(node.id_sec2.to_vec().as_slice());

                res_body.put_slice(node.ip_addr.ip.to_vec().as_slice());

                let port = node.ip_addr.port;
                let port: Vec<u8> = encoder.serialize(&port).unwrap();
                res_body.put_slice(port.as_slice());
            }

            let mut i = 0;
            for c in res_body.iter() {
                debug!("res_body[{}]: {}", i, c);
                i = i + 1;
            }
        }
        None => warn!("Node not found in outbound node list."),
    }

    res_body
}

fn handle_active_nodes_res(req_body: Vec<u8>) {
    let (node_count, rest) = req_body.split_at(1);

    debug!("node_count: {}", node_count[0]);

    let mut node_list = Vec::new();
    let mut i = 0;
    let mut rest = rest;
    while i < node_count[0] as u32 {
        let mut node = Node::new();

        let (id_sec1, rest_body) = rest.split_at(24);
        let (id_sec2, rest_body) = rest_body.split_at(12);
        let (ip, rest_body) = rest_body.split_at(8);
        let (mut port, next) = rest_body.split_at(8);

        node.ip_addr.ip.copy_from_slice(ip.to_vec().as_slice());
        node.ip_addr.port = port.read_u32::<BigEndian>().unwrap();

        node.id_hash = calculate_hash(&node);
        node.id_sec1.copy_from_slice(id_sec1.to_vec().as_slice());
        node.id_sec2.copy_from_slice(id_sec2.to_vec().as_slice());
        node_list.push(node);

        rest = next;
        i = i + 1;
    }

    let temp_nodes = GLOBAL_TEMP_NODES_MAP.get().lock().unwrap();
//    for node in node_list.iter() {
//        temp_nodes.insert(node.id_hash, *node);
//        debug!("Temp {}", node);
//    }
    debug!("temp nodes list size: {}.", temp_nodes.len());
}

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}