use actix::Recipient;
use serde::Serialize;
use web3::types::H160;

use crate::actors::ws::{SubOp, SubOpCode, XWsSub};
use crate::db::StoreDB;
use crate::{actors::ws::WsMessage, datas::BoxedResult};
use std::collections::HashSet;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        RwLock,
    },
};

use super::handle::ProposalState;
pub struct AppData {
    pub store: StoreDB,
    pub support_list: RwLock<HashMap<(String, String), bool>>,
    pub categories: RwLock<Vec<&'static str>>,
    pub liquidity: RwLock<Vec<&'static str>>,
    current_block: AtomicU64,
    pub chain_id: u32,
    proposals: RwLock<HashMap<H160, u64>>,
    proposals_state: RwLock<HashMap<u64, ProposalState>>,
    client_list: RwLock<HashMap<Recipient<WsMessage>, HashSet<XWsSub>>>,
}

impl AppData {
    pub fn new(
        store: StoreDB,
        categories: Vec<&'static str>,
        liquidity: Vec<&'static str>,
        chain_id: u32,
        proposals: Vec<(u64, String, String)>,
    ) -> Self {
        let proposals_state = proposals
            .iter()
            .map(|(id, _, state)| {
                let state = format!("{:?}", state).to_lowercase();
                if let Ok(state) = serde_json::from_str(state.as_str()) {
                    Some((*id, state))
                } else {
                    None
                }
            })
            .flatten()
            .collect::<HashMap<u64, ProposalState>>();
        let proposals = proposals
            .iter()
            .map(|(id, v, _)| {
                if let Ok(a) = v.parse::<H160>() {
                    Some((a, *id))
                } else {
                    None
                }
            })
            .flatten()
            .collect::<HashMap<H160, u64>>();
        Self {
            store,
            support_list: RwLock::new(HashMap::new()),
            categories: RwLock::new(categories),
            liquidity: RwLock::new(liquidity),
            current_block: AtomicU64::new(0),
            chain_id,
            proposals: RwLock::new(proposals),
            proposals_state: RwLock::new(proposals_state),
            client_list: RwLock::new(HashMap::new()),
        }
    }

    pub fn insert_support(&self, addr: String, symbol: String, flag: bool) -> BoxedResult<()> {
        let mut support_list = self.support_list.write().map_err(|e| e.to_string())?;
        support_list.insert((addr, symbol), flag);
        Ok(())
    }

    pub async fn coins_support(&self) -> BoxedResult<Vec<(String, String)>> {
        if let Ok(map) = self.support_list.read() {
            let list = map
                .iter()
                .filter(|(_, &b)| b)
                .map(|(coin, _)| coin.clone())
                .collect::<Vec<(String, String)>>();
            Ok(list)
        } else {
            Err("unknown error".into())
        }
    }
    pub fn get_current_block(&self) -> u64 {
        self.current_block.load(Ordering::Relaxed)
    }

    pub fn set_current_block(&self, block: u64) -> u64 {
        self.current_block.swap(block, Ordering::Relaxed)
    }

    pub fn get_proposals(&self) -> Vec<H160> {
        if let Ok(proposals) = self.proposals.read() {
            proposals.keys().copied().collect()
        } else {
            vec![]
        }
    }

    pub fn get_proposal_id(&self, proposal: &H160) -> Option<u64> {
        if let Ok(proposals) = self.proposals.read() {
            proposals.get(proposal).copied()
        } else {
            None
        }
    }

    pub fn contains_proposal(&self, proposal: &H160) -> bool {
        if let Ok(proposals) = self.proposals.read() {
            proposals.contains_key(proposal)
        } else {
            false
        }
    }

    pub fn set_proposal_state(&self, id: u64, state: ProposalState) {
        if let Ok(mut proposals_state) = self.proposals_state.write() {
            let _old_state = proposals_state.insert(id, state);
        }
    }

    pub fn insert_proposal(&self, proposal: H160, proposal_id: u64) {
        if let Ok(mut contracts) = self.proposals.write() {
            contracts.insert(proposal, proposal_id);
        }
    }

    pub async fn banners(&self) -> BoxedResult<Vec<String>> {
        self.store.query_banner().await
    }
    pub fn client_sub(&self, recipient: Recipient<WsMessage>, sub: XWsSub) -> BoxedResult<()> {
        if let Ok(mut client_list) = self.client_list.write() {
            client_list
                .entry(recipient)
                .or_insert_with(HashSet::new)
                .insert(sub);
            Ok(())
        } else {
            Err("unknown error".into())
        }
    }

    pub fn client_unsub(&self, recipient: Recipient<WsMessage>, sub: &XWsSub) -> BoxedResult<()> {
        if let Ok(mut client_list) = self.client_list.write() {
            client_list.entry(recipient).and_modify(|v| {
                v.remove(sub);
            });
            Ok(())
        } else {
            Err("unknown error".into())
        }
    }

    pub fn delete_client(&self, recipient: &Recipient<WsMessage>) {
        if let Ok(mut client_list) = self.client_list.write() {
            client_list.remove(recipient);
        }
    }

    pub fn push_to_client<T>(&self, target: &str, sub_op: SubOpCode, data: T)
    where
        T: Serialize + Clone,
    {
        if let Ok(client_list) = self.client_list.read() {
            for (client, sub_set) in client_list.iter() {
                for sub in sub_set {
                    if sub.target != target {
                        continue;
                    }
                    if let Ok(res) = serde_json::to_string(&SubOp::new(
                        sub_op,
                        sub.target.clone(),
                        data.clone(),
                        sub.id,
                    )) {
                        if client.do_send(WsMessage(res.clone())).is_err() {
                            continue;
                        }
                    }
                }
            }
        }
    }

    pub fn get_proposal_address(&self, proposal_id: u64) -> Option<H160> {
        if let Ok(proposals) = self.proposals.read() {
            proposals
                .iter()
                .find_map(|(&key, &val)| if val == proposal_id { Some(key) } else { None })
        } else {
            None
        }
    }
}
