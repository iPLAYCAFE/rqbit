use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};

use anyhow::Context;
use buffers::ByteBufOwned;
use futures::{Stream, StreamExt, stream::FuturesUnordered};
use librqbit_core::torrent_metainfo::TorrentMetaV1Info;
use tracing::{Instrument, debug, debug_span, info};

use crate::{
    peer_connection::PeerConnectionOptions, peer_info_reader, spawn_utils::BlockingSpawner,
    stream_connect::StreamConnector,
};
use librqbit_core::hash_id::Id20;

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum ReadMetainfoResult<Rx> {
    Found {
        info: TorrentMetaV1Info<ByteBufOwned>,
        info_bytes: ByteBufOwned,
        rx: Rx,
        seen: HashSet<SocketAddr>,
    },
    ChannelClosed {
        #[allow(dead_code)]
        seen: HashSet<SocketAddr>,
    },
}

pub async fn read_metainfo_from_peer_receiver<A: Stream<Item = SocketAddr> + Unpin>(
    peer_id: Id20,
    info_hash: Id20,
    initial_addrs: Vec<SocketAddr>,
    addrs_stream: A,
    peer_connection_options: Option<PeerConnectionOptions>,
    connector: Arc<StreamConnector>,
    client_name_and_version: String,
) -> ReadMetainfoResult<A> {
    let mut seen = HashSet::<SocketAddr>::new();
    let mut in_flight = HashSet::<SocketAddr>::new();
    let mut last_attempt = HashMap::<SocketAddr, tokio::time::Instant>::new();
    let mut attempt_counts = HashMap::<SocketAddr, usize>::new();
    let mut addrs = addrs_stream;

    let semaphore = tokio::sync::Semaphore::new(128);
    const RETRY_COOLDOWN: Duration = Duration::from_secs(5);
    const MAX_RETRIES_AFTER_COMPLETION: usize = 6;

    let read_info_guarded = |addr: SocketAddr| {
        let semaphore = &semaphore;
        let connector = connector.clone();
        let client_name_and_version = client_name_and_version.clone();
        async move {
            let token = semaphore.acquire().await.ok();
            let ret = peer_info_reader::read_metainfo_from_peer(
                addr,
                peer_id,
                info_hash,
                peer_connection_options,
                // This shouldn't be called anyway as we aren't reading/writing to disk, so it's
                // ok not to use a shared one.
                BlockingSpawner::new(1),
                connector,
                client_name_and_version,
            )
            .instrument(debug_span!("read_metainfo_from_peer", ?addr))
            .await
            .with_context(|| format!("error reading metainfo from {addr}"));
            drop(token);
            (addr, ret)
        }
    };

    let mut unordered = FuturesUnordered::new();

    for a in initial_addrs {
        seen.insert(a);
        in_flight.insert(a);
        last_attempt.insert(a, tokio::time::Instant::now());
        *attempt_counts.entry(a).or_insert(0) += 1;
        unordered.push(read_info_guarded(a));
    }

    let mut addrs_completed = false;

    loop {
        if addrs_completed && unordered.is_empty() {
            let any_can_retry = seen.iter().any(|a| {
                let attempts = attempt_counts.get(a).copied().unwrap_or(0);
                attempts < MAX_RETRIES_AFTER_COMPLETION
            });
            if !any_can_retry {
                return ReadMetainfoResult::ChannelClosed { seen };
            }
        }

        let now = tokio::time::Instant::now();
        let next_retry_at = seen
            .iter()
            .filter(|a| !in_flight.contains(a))
            .filter(|a| {
                if addrs_completed {
                    let attempts = attempt_counts.get(a).copied().unwrap_or(0);
                    attempts < MAX_RETRIES_AFTER_COMPLETION
                } else {
                    true
                }
            })
            .map(|a| match last_attempt.get(a) {
                Some(t) => *t + RETRY_COOLDOWN,
                None => now,
            })
            .min();

        let has_retry = next_retry_at.is_some();
        let sleep_until = next_retry_at.unwrap_or(now + Duration::from_secs(86400));

        tokio::select! {
            done = unordered.next(), if !unordered.is_empty() => {
                match done {
                    Some((addr, Ok((info, info_bytes)))) => {
                        info!(?addr, "successfully fetched metainfo from peer");
                        return ReadMetainfoResult::Found { info, info_bytes, seen, rx: addrs };
                    }
                    Some((addr, Err(e))) => {
                        debug!(?addr, "failed reading metainfo from peer: {:#}", e);
                        in_flight.remove(&addr);
                        last_attempt.insert(addr, tokio::time::Instant::now());
                    }
                    None => unreachable!()
                }
            }

            next_addr = addrs.next(), if !addrs_completed => {
                match next_addr {
                    Some(addr) => {
                        seen.insert(addr);
                        if !in_flight.contains(&addr) {
                            let can_start = match last_attempt.get(&addr) {
                                Some(t) => t.elapsed() >= RETRY_COOLDOWN,
                                None => true,
                            };
                            if can_start {
                                in_flight.insert(addr);
                                last_attempt.insert(addr, tokio::time::Instant::now());
                                *attempt_counts.entry(addr).or_insert(0) += 1;
                                unordered.push(read_info_guarded(addr));
                            }
                        }
                    }
                    None => {
                        addrs_completed = true;
                    }
                }
            }

            _ = tokio::time::sleep_until(sleep_until), if has_retry => {
                let now = tokio::time::Instant::now();
                for &a in &seen {
                    if !in_flight.contains(&a) {
                        let attempts = attempt_counts.get(&a).copied().unwrap_or(0);
                        if addrs_completed && attempts >= MAX_RETRIES_AFTER_COMPLETION {
                            continue;
                        }
                        let can_retry = match last_attempt.get(&a) {
                            Some(t) => now.duration_since(*t) >= RETRY_COOLDOWN,
                            None => true,
                        };
                        if can_retry {
                            in_flight.insert(a);
                            last_attempt.insert(a, now);
                            *attempt_counts.entry(a).or_insert(0) += 1;
                            debug!(?a, "retrying peer for metainfo");
                            unordered.push(read_info_guarded(a));
                        }
                    }
                }
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use dht::{DhtBuilder, Id20};
    use librqbit_core::peer_id::generate_peer_id;

    use super::*;
    use std::{
        str::FromStr,
        sync::{Arc, Once},
    };

    static LOG_INIT: Once = Once::new();

    fn init_logging() {
        #[allow(unused_must_use)]
        LOG_INIT.call_once(|| {
            // pretty_env_logger::try_init();
        })
    }

    #[tokio::test]
    #[ignore]
    async fn read_metainfo_from_dht() {
        init_logging();

        let info_hash = Id20::from_str("cab507494d02ebb1178b38f2e9d7be299c86b862").unwrap();
        let dht = DhtBuilder::new().await.unwrap();

        let peer_rx = dht.get_peers(info_hash, None);
        let peer_id = generate_peer_id(b"-xx1234-");
        match read_metainfo_from_peer_receiver(
            peer_id,
            info_hash,
            Vec::new(),
            peer_rx,
            None,
            Arc::new(StreamConnector::new(Default::default()).await.unwrap()),
            crate::client_name_and_version().to_owned(),
        )
        .await
        {
            ReadMetainfoResult::Found { info, .. } => dbg!(info),
            ReadMetainfoResult::ChannelClosed { .. } => todo!("should not have happened"),
        };
    }
}
