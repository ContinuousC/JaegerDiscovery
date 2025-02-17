/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::marker::PhantomData;

use reqwest::Client;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use url::Url;

use crate::error::Error;

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct QueryResponse<T, S> {
    pub(crate) hits: Hits<T, S>,
    #[serde(default)]
    pub(crate) pit_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Hits<T, S> {
    pub(crate) hits: Vec<Hit<T, S>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Hit<T, S> {
    #[serde(rename = "_index")]
    pub(crate) index: String,
    #[serde(rename = "_source")]
    pub(crate) source: T,
    pub sort: Option<S>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct PitResponse {
    pub(crate) pit_id: String,
}

#[derive(Deserialize, Debug)]
pub(crate) struct DeletePitResponse {
    pub(crate) pits: Vec<DeletePitAction>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct DeletePitAction {
    pub(crate) successful: bool,
    // pub(crate) pit_id: String,
}

#[derive(Clone)]
pub(crate) struct EsPit<'a> {
    client: &'a Client,
    keep_alive: &'a str,
    url: Url,
    pit_id: Option<String>,
}

impl<'a> EsPit<'a> {
    pub(crate) async fn new(
        client: &'a Client,
        base: &Url,
        index_pattern: &str,
        keep_alive: &'a str,
    ) -> Result<Self, Error> {
        let res = client
            .post(base.join(&format!("{index_pattern}/_search/point_in_time"))?)
            .query(&json!({"keep_alive": keep_alive}))
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(Error::Reqwest)?
            .json::<PitResponse>()
            .await
            .map_err(Error::Reqwest)?;
        Ok(Self {
            client,
            keep_alive,
            url: base.clone(),
            pit_id: Some(res.pit_id),
        })
    }

    pub(crate) fn query<'b, T, S, L, U>(
        &'b mut self,
        query: T,
        sort: Option<S>,
        last: Option<L>,
        batch_size: u64,
    ) -> EsQuery<'a, 'b, T, S, L, U>
    where
        'a: 'b,
        T: Serialize,
        S: Serialize,
        L: Serialize + DeserializeOwned + Clone,
        U: DeserializeOwned,
    {
        EsQuery {
            pit: self,
            batch_size,
            query,
            sort,
            last,
            marker: PhantomData,
        }
    }

    pub(crate) async fn delete(mut self) -> Result<(), Error> {
        if let Some(pit_id) = self.pit_id.take() {
            let res = self
                .client
                .delete(self.url.join("_search/point_in_time")?)
                .json(&json!({ "pit_id": [pit_id] }))
                .send()
                .await
                .and_then(|r| r.error_for_status())
                .map_err(Error::Reqwest)?
                .json::<DeletePitResponse>()
                .await
                .map_err(Error::Reqwest)?;
            res.pits
                .into_iter()
                .try_for_each(|pit| pit.successful.then_some(()).ok_or(Error::DeletePit))?
        }
        Ok(())
    }
}

impl Drop for EsPit<'_> {
    fn drop(&mut self) {
        if self.pit_id.is_some() {
            log::warn!("Elasticsearch PIT left open; use pit.delete().await");
        }
    }
}

pub(crate) struct EsQuery<'a: 'b, 'b, T, S, L, U> {
    pit: &'b mut EsPit<'a>,
    batch_size: u64,
    query: T,
    sort: Option<S>,
    last: Option<L>,
    marker: PhantomData<U>,
}

#[derive(Serialize, Debug)]
struct PitQuery<'a, T, S, L> {
    query: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    sort: Option<&'a S>,
    #[serde(skip_serializing_if = "Option::is_none")]
    search_after: Option<&'a L>,
    size: u64,
    pit: QueryPit<'a>,
}

#[derive(Serialize, Debug)]
struct QueryPit<'a> {
    id: &'a str,
    keep_alive: &'a str,
}

impl<'a: 'b, 'b, T, S, L, U> EsQuery<'a, 'b, T, S, L, U>
where
    T: Serialize,
    S: Serialize,
    L: Serialize + DeserializeOwned + Clone,
    U: DeserializeOwned,
{
    pub(crate) async fn next(&mut self) -> Result<Option<QueryResponse<U, L>>, Error> {
        log::debug!(
            "Query: last = {}",
            serde_json::to_string(&self.last).unwrap()
        );

        let pit_id = match self.pit.pit_id.as_deref() {
            Some(id) => id,
            None => return Ok(None),
        };

        let res = self
            .pit
            .client
            .post(self.pit.url.join("_search")?)
            .json(&PitQuery {
                query: &self.query,
                sort: self.sort.as_ref(),
                search_after: self.last.as_ref(),
                size: self.batch_size,
                pit: QueryPit {
                    id: pit_id,
                    keep_alive: self.pit.keep_alive,
                },
            })
            .send()
            .await?;
        if res.status().is_success() {
            let res = res
                .json::<QueryResponse<U, L>>()
                .await
                .map_err(Error::Reqwest)?;
            self.pit.pit_id = res.pit_id.clone();
            self.last = res.hits.hits.last().and_then(|hit| hit.sort.clone());
            Ok((!res.hits.hits.is_empty()).then_some(res))
        } else {
            let err = res.error_for_status_ref().unwrap_err();
            let msg = res.json::<serde_json::Value>().await?;
            log::debug!(
                "error response: {}",
                serde_json::to_string_pretty(&msg).unwrap()
            );
            Err(err.into())
        }
    }
}
