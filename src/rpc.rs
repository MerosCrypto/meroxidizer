use crate::cli::Opts;
use eyre::Report;
use log::error;
use reqwest::blocking::Client;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{fmt, time::Duration};

const RETRY_BACKOFF: Duration = Duration::from_secs(1);

#[derive(Deserialize, Debug)]
pub struct RpcError {
    pub code: i64,
    #[serde(default)]
    pub message: Option<String>,
}

impl fmt::Display for RpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "code {}", self.code)?;
        if let Some(message) = &self.message {
            write!(f, " message {}", message)?;
        }
        Ok(())
    }
}

impl std::error::Error for RpcError {}

#[derive(Serialize)]
struct FullRequest<'a, P> {
    #[serde(rename = "jsonrpc")]
    json_rpc: &'static str,
    id: i64,
    method: &'a str,
    params: P,
}

#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(untagged)]
enum FullResponse<R> {
    Error { error: RpcError },
    Result { id: i64, result: R },
}

#[derive(Debug, Deserialize)]
pub struct RpcMiningTarget {
    pub id: i64,
    #[serde(with = "hex")]
    pub key: [u8; 32],
    #[serde(with = "hex")]
    pub header: Vec<u8>,
    pub difficulty: u64,
}

pub struct Rpc {
    pub opts: Opts,
}

impl Rpc {
    fn with_retry<F, O>(&mut self, mut f: F) -> O
    where
        F: FnMut(&mut Self) -> Result<O, Report>,
    {
        loop {
            match f(self) {
                Ok(x) => return x,
                Err(err) => {
                    error!("error making RPC call: {:#}", err);
                }
            }
            std::thread::sleep(RETRY_BACKOFF);
        }
    }

    pub fn single_request<P: Serialize + Clone, R: DeserializeOwned>(
        &mut self,
        method: &str,
        params: P,
    ) -> Result<R, RpcError> {
        self.with_retry(|rpc| {
            let res: FullResponse::<R> = serde_json::from_str(
                &Client::new()
                .post(&("http://".to_string() + &rpc.opts.rpc))
                .bearer_auth(&rpc.opts.token)
                .json(&FullRequest {
                    json_rpc: "2.0",
                    method,
                    id: 0,
                    params: params.clone(),
                })
                .send()?
                .text()?
            )?;
            match res {
                FullResponse::Error { error, .. } => Ok(Err(error)),
                FullResponse::Result { result, .. } => Ok(Ok(result)),
            }
        })
    }

    pub fn get_height(&mut self) -> usize {
        self.with_retry(|rpc| {
            let res: FullResponse::<usize> = serde_json::from_str(
                &Client::new()
                .post(&("http://".to_string() + &rpc.opts.rpc))
                .bearer_auth(&rpc.opts.token)
                .json(&FullRequest {
                    json_rpc: "2.0",
                    method: "merit_getHeight",
                    id: 0,
                    params: serde_json::json!({}),
                })
                .send()?
                .text()?
            )?;
            match res {
                FullResponse::Error { error, .. } => Err(error.into()),
                FullResponse::Result { result, .. } => Ok(result),
            }
        })
    }

    pub fn get_mining_target(&mut self, miner_pubkey: &str) -> RpcMiningTarget {
        self.with_retry(|rpc| {
            let res: FullResponse::<RpcMiningTarget> = serde_json::from_str(
                &Client::new()
                .post(&("http://".to_string() + &rpc.opts.rpc))
                .bearer_auth(&rpc.opts.token)
                .json(&FullRequest {
                    json_rpc: "2.0",
                    method: "merit_getBlockTemplate",
                    id: 0,
                    params: serde_json::json!({"miner": miner_pubkey}),
                })
                .send()?
                .text()?
            )?;
            match res {
                FullResponse::Error { error, .. } => Err(error.into()),
                FullResponse::Result { result, .. } => Ok(result),
            }
        })
    }
}
